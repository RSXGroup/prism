//! Decoding and encoding, kept apart from the algorithm on purpose.
//!
//! `prism-core` never sees a file. Everything format-specific lives here so the
//! browser build can swap in its own codecs without touching the repair pass.

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use image::{ImageEncoder, ImageReader, RgbaImage};

/// An output format prism knows how to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    #[cfg(feature = "png")]
    Png,
    #[cfg(feature = "webp")]
    Webp,
    #[cfg(feature = "avif")]
    Avif,
}

impl Format {
    /// Matches a file extension, case-insensitively.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            #[cfg(feature = "png")]
            "png" => Some(Self::Png),
            #[cfg(feature = "webp")]
            "webp" => Some(Self::Webp),
            #[cfg(feature = "avif")]
            "avif" => Some(Self::Avif),
            _ => None,
        }
    }
}

/// Every extension this build accepts, for help text and folder scanning.
pub const SUPPORTED: &[&str] = &[
    #[cfg(feature = "png")]
    "png",
    #[cfg(feature = "webp")]
    "webp",
    #[cfg(feature = "avif")]
    "avif",
];

/// True if the path carries an extension this build can handle.
pub fn is_supported(path: &Path) -> bool {
    Format::from_path(path).is_some()
}

/// Decodes any supported file to a non-premultiplied RGBA8 buffer.
///
/// 16-bit sources are reduced to 8 bits here rather than being reinterpreted as
/// raw bytes — the old pipeline read them as 8-bit and produced garbage.
pub fn decode(path: &Path) -> Result<RgbaImage> {
    let reader = ImageReader::open(path)
        .with_context(|| format!("cannot open {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("cannot identify the format of {}", path.display()))?;

    let image = reader
        .decode()
        .with_context(|| format!("cannot decode {}", path.display()))?;

    Ok(image.to_rgba8())
}

/// Encodes and writes `image`, choosing the format from `path`'s extension.
///
/// The write goes to a sibling temporary file and is renamed into place, so an
/// interrupted run can never leave a half-written image where the original was.
pub fn encode_to_file(image: &RgbaImage, path: &Path) -> Result<()> {
    let format =
        Format::from_path(path).ok_or_else(|| anyhow!("no encoder for {}", path.display()))?;

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }

    let tmp = temp_sibling(path);
    let result = (|| -> Result<()> {
        let file =
            fs::File::create(&tmp).with_context(|| format!("cannot create {}", tmp.display()))?;
        let mut out = BufWriter::new(file);
        encode_into(image, format, &mut out)?;
        out.into_inner()
            .context("cannot flush the encoded image")?
            .sync_all()
            .context("cannot sync the encoded image to disk")?;
        Ok(())
    })();

    if let Err(err) = result {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }

    fs::rename(&tmp, path).with_context(|| {
        format!(
            "cannot move {} into place at {}",
            tmp.display(),
            path.display()
        )
    })?;

    Ok(())
}

fn encode_into<W: std::io::Write>(image: &RgbaImage, format: Format, out: &mut W) -> Result<()> {
    let (w, h) = image.dimensions();
    let raw = image.as_raw();

    match format {
        #[cfg(feature = "png")]
        Format::Png => {
            use image::codecs::png::{CompressionType, FilterType, PngEncoder};
            PngEncoder::new_with_quality(out, CompressionType::Best, FilterType::Adaptive)
                .write_image(raw, w, h, image::ExtendedColorType::Rgba8)
                .context("PNG encoding failed")?;
        }
        #[cfg(feature = "webp")]
        Format::Webp => {
            use image::codecs::webp::WebPEncoder;
            // Lossless only. Lossy WebP would re-introduce colour error in the
            // exact channels we just spent the whole pipeline repairing.
            WebPEncoder::new_lossless(out)
                .write_image(raw, w, h, image::ExtendedColorType::Rgba8)
                .context("WebP encoding failed")?;
        }
        #[cfg(feature = "avif")]
        Format::Avif => {
            use image::codecs::avif::AvifEncoder;
            AvifEncoder::new_with_speed_quality(out, 4, 100)
                .write_image(raw, w, h, image::ExtendedColorType::Rgba8)
                .context("AVIF encoding failed")?;
        }
    }

    Ok(())
}

/// A temp path beside the destination, so the rename stays on one filesystem.
fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".prism-{}.tmp", std::process::id()));
    path.with_file_name(name)
}

/// Walks `root`, yielding files this build can decode.
pub fn scan_dir(root: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let depth = if recursive { usize::MAX } else { 1 };
    let mut found = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .max_depth(depth)
        .follow_links(false)
    {
        let entry = entry.with_context(|| format!("cannot read {}", root.display()))?;
        if entry.file_type().is_file() && is_supported(entry.path()) {
            found.push(entry.into_path());
        }
    }

    if found.is_empty() && !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }

    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_extensions_case_insensitively() {
        assert!(is_supported(Path::new("a.png")));
        assert!(is_supported(Path::new("a.PNG")));
        assert!(is_supported(Path::new("dir.with.dots/a.WebP")));
        assert!(!is_supported(Path::new("a.jpg")));
        assert!(!is_supported(Path::new("png")));
        assert!(!is_supported(Path::new("a.png.bak")));
    }

    #[test]
    fn temp_file_stays_in_the_destination_directory() {
        let dst = Path::new("/some/where/sprite.png");
        let tmp = temp_sibling(dst);
        assert_eq!(tmp.parent(), dst.parent());
        assert_ne!(tmp, dst);
    }
}
