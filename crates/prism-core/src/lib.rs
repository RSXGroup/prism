//! # prism-core
//!
//! Probably Repairs Inconsistent Semi-transparency.
//!
//! Most image editors write black into the RGB channels of fully transparent
//! pixels. You cannot see it — alpha is zero — but GPUs do not sample alpha and
//! colour independently. Bilinear filtering and mipmap generation blend the
//! hidden RGB back in, and a sprite that looked clean in Photoshop gets a dark
//! halo in-engine.
//!
//! The fix is to replace that hidden RGB with the colour of the nearest visible
//! pixel, leaving alpha exactly as it was. See [`fill_rgba`].
//!
//! ```
//! use prism_core::{FillOptions, fill_rgba};
//!
//! // 2×1 RGBA: an opaque red pixel and a transparent black one.
//! let mut pixels = vec![255, 0, 0, 255, /**/ 0, 0, 0, 0];
//! let stats = fill_rgba(&mut pixels, 2, 1, FillOptions::default())?;
//!
//! assert_eq!(stats.filled, 1);
//! assert_eq!(&pixels[4..8], &[255, 0, 0, 0]); // red now, still invisible
//! # Ok::<(), prism_core::FillError>(())
//! ```
//!
//! This crate is deliberately free of image codecs and I/O so the exact same
//! code runs in the CLI and compiled to WebAssembly in the browser. Callers
//! bring their own decoded RGBA8 buffer.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod edt;
pub mod fill;

pub use edt::{NO_SITE, feature_transform};
pub use fill::{FillError, FillOptions, FillStats, fill_rgba};

/// The crate version, handy for CLI and wasm banner strings.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
