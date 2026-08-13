//! The repair pass itself: rewrite the RGB hiding under transparent pixels.

use crate::edt::{NO_SITE, feature_transform};
use core::fmt;

/// Which pixels are trusted as colour sources, and which get rewritten.
///
/// The two thresholds carve the alpha range into three bands:
///
/// | alpha                              | role                          |
/// |------------------------------------|-------------------------------|
/// | `>= seed_alpha_min`                | trusted source, never touched |
/// | `<= fill_alpha_max`                | RGB rewritten from nearest source |
/// | in between                         | left completely alone         |
///
/// The defaults (`255` / `0`) mean only fully opaque pixels seed the fill and
/// only fully transparent ones are rewritten. That middle band matters: a pixel
/// at `alpha = 1` whose RGB an editor already crushed to black is exactly the
/// garbage we are trying to remove, so it must not be allowed to seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillOptions {
    /// Minimum alpha for a pixel to be trusted as a colour source.
    pub seed_alpha_min: u8,
    /// Maximum alpha for a pixel to have its RGB rewritten.
    pub fill_alpha_max: u8,
}

impl Default for FillOptions {
    fn default() -> Self {
        Self {
            seed_alpha_min: 255,
            fill_alpha_max: 0,
        }
    }
}

/// What a [`fill_rgba`] call actually did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FillStats {
    /// Pixels whose RGB was rewritten.
    pub filled: u64,
    /// Pixels that acted as colour sources.
    pub seeds: u64,
    /// Pixels in neither band, left untouched.
    pub untouched_partial: u64,
}

impl FillStats {
    /// True when the image needed no work — nothing to fill, or nothing to fill *from*.
    pub fn is_noop(&self) -> bool {
        self.filled == 0
    }
}

/// Why a fill could not be attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillError {
    /// `pixels.len()` disagrees with `width * height * 4`.
    BufferSize {
        /// Byte count implied by the given dimensions.
        expected: usize,
        /// Byte count actually supplied.
        got: usize,
    },
    /// The seed and fill bands overlap, so a pixel could be both source and target.
    OverlappingThresholds {
        /// The offending [`FillOptions::seed_alpha_min`].
        seed_alpha_min: u8,
        /// The offending [`FillOptions::fill_alpha_max`].
        fill_alpha_max: u8,
    },
    /// Width or height exceeds what a `u32` pixel index can address.
    TooLarge {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
    },
}

impl fmt::Display for FillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferSize { expected, got } => {
                write!(
                    f,
                    "pixel buffer is {got} bytes, expected {expected} (RGBA8)"
                )
            }
            Self::OverlappingThresholds {
                seed_alpha_min,
                fill_alpha_max,
            } => write!(
                f,
                "seed alpha ({seed_alpha_min}) must be greater than fill alpha ({fill_alpha_max})",
            ),
            Self::TooLarge { width, height } => {
                write!(f, "image {width}x{height} is too large to index")
            }
        }
    }
}

impl core::error::Error for FillError {}

/// Repairs an 8-bit RGBA buffer in place.
///
/// Every pixel in the fill band takes the RGB of its nearest source pixel by
/// Euclidean distance. **Alpha is never modified** — the image looks identical,
/// but the colour data that bilinear filtering and mipmap generation drag out
/// from under transparent texels is now the right colour instead of black.
pub fn fill_rgba(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    opts: FillOptions,
) -> Result<FillStats, FillError> {
    if opts.seed_alpha_min <= opts.fill_alpha_max {
        return Err(FillError::OverlappingThresholds {
            seed_alpha_min: opts.seed_alpha_min,
            fill_alpha_max: opts.fill_alpha_max,
        });
    }

    let (w, h) = (width as usize, height as usize);
    let count = w
        .checked_mul(h)
        .filter(|&n| n <= u32::MAX as usize)
        .ok_or(FillError::TooLarge { width, height })?;

    let expected = count * 4;
    if pixels.len() != expected {
        return Err(FillError::BufferSize {
            expected,
            got: pixels.len(),
        });
    }

    if count == 0 {
        return Ok(FillStats::default());
    }

    // ── Classify every pixel by its alpha band ───────────────────────────────
    let mut is_site = vec![false; count];
    let mut stats = FillStats::default();

    for i in 0..count {
        let alpha = pixels[i * 4 + 3];
        if alpha >= opts.seed_alpha_min {
            is_site[i] = true;
            stats.seeds += 1;
        } else if alpha > opts.fill_alpha_max {
            stats.untouched_partial += 1;
        }
    }

    let targets = count as u64 - stats.seeds - stats.untouched_partial;
    if stats.seeds == 0 || targets == 0 {
        // Fully opaque, fully transparent, or nothing trustworthy to copy from.
        return Ok(stats);
    }

    // ── Resolve nearest source for every pixel, then copy colour across ──────
    let nearest = feature_transform(&is_site, w, h);

    for i in 0..count {
        if pixels[i * 4 + 3] > opts.fill_alpha_max {
            continue;
        }
        let src = nearest[i];
        if src == NO_SITE {
            continue;
        }

        let s = src as usize * 4;
        let (r, g, b) = (pixels[s], pixels[s + 1], pixels[s + 2]);

        let d = i * 4;
        pixels[d] = r;
        pixels[d + 1] = g;
        pixels[d + 2] = b;
        // Alpha at d + 3 is deliberately left as-is.

        stats.filled += 1;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an RGBA buffer from `(r, g, b, a)` tuples.
    fn buf(px: &[(u8, u8, u8, u8)]) -> Vec<u8> {
        px.iter().flat_map(|&(r, g, b, a)| [r, g, b, a]).collect()
    }

    #[test]
    fn bleeds_opaque_colour_into_transparent_neighbours() {
        // 2×1: a red opaque pixel beside a black transparent one.
        let mut pixels = buf(&[(255, 0, 0, 255), (0, 0, 0, 0)]);
        let stats = fill_rgba(&mut pixels, 2, 1, FillOptions::default()).unwrap();

        assert_eq!(stats.filled, 1);
        assert_eq!(stats.seeds, 1);
        assert_eq!(&pixels[4..7], &[255, 0, 0], "RGB should copy from the seed");
        assert_eq!(pixels[7], 0, "alpha must stay untouched");
    }

    #[test]
    fn never_touches_the_opaque_pixels() {
        let mut pixels = buf(&[(10, 20, 30, 255), (0, 0, 0, 0)]);
        fill_rgba(&mut pixels, 2, 1, FillOptions::default()).unwrap();
        assert_eq!(&pixels[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn semi_transparent_pixels_do_not_seed_by_default() {
        // The alpha=1 black pixel is the corrupted data — it must not be a source.
        // Layout: [black a=1] [transparent] [green opaque]
        let mut pixels = buf(&[(0, 0, 0, 1), (0, 0, 0, 0), (0, 255, 0, 255)]);
        let stats = fill_rgba(&mut pixels, 3, 1, FillOptions::default()).unwrap();

        assert_eq!(stats.seeds, 1, "only the opaque pixel may seed");
        assert_eq!(stats.untouched_partial, 1);
        assert_eq!(
            &pixels[4..7],
            &[0, 255, 0],
            "should pull green from the far opaque pixel, not black from the near one",
        );
        // The semi-transparent pixel itself is left alone at the default thresholds.
        assert_eq!(&pixels[0..4], &[0, 0, 0, 1]);
    }

    #[test]
    fn raising_fill_alpha_max_also_repairs_semi_transparent_pixels() {
        let mut pixels = buf(&[(0, 0, 0, 1), (0, 0, 0, 0), (0, 255, 0, 255)]);
        let opts = FillOptions {
            seed_alpha_min: 255,
            fill_alpha_max: 128,
        };
        let stats = fill_rgba(&mut pixels, 3, 1, opts).unwrap();

        assert_eq!(stats.filled, 2);
        assert_eq!(stats.untouched_partial, 0);
        assert_eq!(&pixels[0..4], &[0, 255, 0, 1], "RGB fixed, alpha preserved");
    }

    #[test]
    fn fully_opaque_image_is_a_noop() {
        let mut pixels = buf(&[(1, 2, 3, 255), (4, 5, 6, 255)]);
        let before = pixels.clone();
        let stats = fill_rgba(&mut pixels, 2, 1, FillOptions::default()).unwrap();

        assert!(stats.is_noop());
        assert_eq!(pixels, before);
    }

    #[test]
    fn fully_transparent_image_is_a_noop() {
        let mut pixels = buf(&[(9, 9, 9, 0), (9, 9, 9, 0)]);
        let before = pixels.clone();
        let stats = fill_rgba(&mut pixels, 2, 1, FillOptions::default()).unwrap();

        assert!(stats.is_noop());
        assert_eq!(pixels, before, "no seeds means nothing to copy");
    }

    #[test]
    fn corner_seeds_are_not_skipped() {
        // Regression guard: the previous implementation ignored the outermost
        // ring when collecting seeds, so an edge-hugging sprite lost its colour.
        // 3×3, only the top-left corner is opaque.
        let mut px = vec![(0u8, 0, 0, 0); 9];
        px[0] = (7, 8, 9, 255);
        let mut pixels = buf(&px);

        let stats = fill_rgba(&mut pixels, 3, 3, FillOptions::default()).unwrap();
        assert_eq!(stats.filled, 8);
        for i in 1..9 {
            assert_eq!(&pixels[i * 4..i * 4 + 3], &[7, 8, 9], "pixel {i}");
        }
    }

    #[test]
    fn rejects_a_mismatched_buffer() {
        let mut pixels = vec![0u8; 7];
        assert!(matches!(
            fill_rgba(&mut pixels, 2, 1, FillOptions::default()),
            Err(FillError::BufferSize {
                expected: 8,
                got: 7
            }),
        ));
    }

    #[test]
    fn rejects_overlapping_thresholds() {
        let mut pixels = vec![0u8; 4];
        let opts = FillOptions {
            seed_alpha_min: 100,
            fill_alpha_max: 100,
        };
        assert!(matches!(
            fill_rgba(&mut pixels, 1, 1, opts),
            Err(FillError::OverlappingThresholds { .. }),
        ));
    }
}
