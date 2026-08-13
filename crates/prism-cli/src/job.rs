//! Per-file work: decide the destination, run the repair, write the result.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use prism_core::{FillOptions, fill_rgba};

use crate::codec;

/// Settings shared by every file in a run.
pub struct Plan<'a> {
    /// Inserted before the extension. Empty means "no suffix".
    pub suffix: &'a str,
    /// Redirects output into this directory, keeping the file name.
    pub out_dir: Option<&'a Path>,
    /// Overwrite sources rather than writing new files.
    pub in_place: bool,
    /// Compute everything, write nothing.
    pub dry_run: bool,
    pub opts: FillOptions,
}

/// What happened to one file.
#[derive(Debug)]
pub enum Outcome {
    /// Colour was bled into `filled` pixels and the result written to `output`.
    Repaired { filled: u64, output: PathBuf },
    /// The image had nothing to repair, so no file was written.
    Clean,
}

/// Decodes, repairs and writes a single image.
pub fn process(input: &Path, plan: &Plan<'_>) -> Result<Outcome> {
    let mut image = codec::decode(input)?;
    let (width, height) = image.dimensions();

    let stats = fill_rgba(image.as_mut(), width, height, plan.opts)
        .with_context(|| format!("cannot repair {}", input.display()))?;

    if stats.is_noop() {
        return Ok(Outcome::Clean);
    }

    let output = destination(input, plan);
    if !plan.dry_run {
        codec::encode_to_file(&image, &output)?;
    }

    Ok(Outcome::Repaired {
        filled: stats.filled,
        output,
    })
}

/// Works out where a repaired image should land.
pub fn destination(input: &Path, plan: &Plan<'_>) -> PathBuf {
    if plan.in_place {
        return input.to_path_buf();
    }

    let name = suffixed_name(input, plan.suffix);
    match plan.out_dir {
        Some(dir) => dir.join(name),
        None => input.with_file_name(name),
    }
}

/// `sprite.png` + `fixed` -> `sprite.fixed.png`.
fn suffixed_name(input: &Path, suffix: &str) -> OsString {
    let file_name = input.file_name().unwrap_or_default().to_os_string();
    if suffix.is_empty() {
        return file_name;
    }

    let Some(stem) = input.file_stem() else {
        return file_name;
    };

    let mut name = stem.to_os_string();
    name.push(".");
    name.push(suffix);
    if let Some(ext) = input.extension() {
        name.push(".");
        name.push(ext);
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan<'a>(suffix: &'a str, out_dir: Option<&'a Path>, in_place: bool) -> Plan<'a> {
        Plan {
            suffix,
            out_dir,
            in_place,
            dry_run: false,
            opts: FillOptions::default(),
        }
    }

    #[test]
    fn inserts_the_suffix_before_the_extension() {
        let got = destination(Path::new("art/sprite.png"), &plan("fixed", None, false));
        assert_eq!(got, PathBuf::from("art/sprite.fixed.png"));
    }

    #[test]
    fn an_empty_suffix_keeps_the_name() {
        let got = destination(Path::new("art/sprite.png"), &plan("", None, false));
        assert_eq!(got, PathBuf::from("art/sprite.png"));
    }

    #[test]
    fn out_dir_relocates_but_keeps_the_suffix() {
        let out = PathBuf::from("build");
        let got = destination(
            Path::new("art/sprite.png"),
            &plan("fixed", Some(&out), false),
        );
        assert_eq!(got, PathBuf::from("build/sprite.fixed.png"));
    }

    #[test]
    fn in_place_overwrites_the_source() {
        let got = destination(Path::new("art/sprite.png"), &plan("fixed", None, true));
        assert_eq!(got, PathBuf::from("art/sprite.png"));
    }

    #[test]
    fn handles_names_with_several_dots() {
        let got = destination(
            Path::new("ui.button.hover.png"),
            &plan("fixed", None, false),
        );
        assert_eq!(got, PathBuf::from("ui.button.hover.fixed.png"));
    }
}
