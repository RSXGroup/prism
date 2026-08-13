//! WebAssembly bindings for the repair pass.
//!
//! Only the algorithm crosses into the browser. Decoding and encoding stay on
//! the JavaScript side, which matters more than it looks: **do not decode via
//! `<canvas>`**. Canvas stores premultiplied alpha, so the round trip through
//! `getImageData` destroys the RGB precision of exactly the semi-transparent
//! pixels this tool exists to repair. Use a real decoder (`@jsquash/png`,
//! `@jsquash/webp`) and hand the raw RGBA buffer to [`repair`].
//!
//! ```js
//! import init, { repair } from "@rsxgroup/prism-wasm";
//! import { decode, encode } from "@jsquash/png";
//!
//! await init();
//! const image = await decode(await file.arrayBuffer()); // { data, width, height }
//! const stats = repair(image.data, image.width, image.height, 255, 0);
//! const png = await encode(image);
//! ```

use prism_core::{FillOptions, fill_rgba};
use wasm_bindgen::prelude::*;

/// Result of a [`repair`] call, readable from JS as a plain object.
#[wasm_bindgen(getter_with_clone)]
#[derive(Debug, Clone, Copy)]
pub struct Stats {
    /// Pixels whose hidden RGB was rewritten.
    pub filled: u32,
    /// Pixels that acted as colour sources.
    pub seeds: u32,
    /// Semi-transparent pixels left alone at the current thresholds.
    pub untouched: u32,
}

/// Repairs an RGBA8 buffer in place and reports what changed.
///
/// `pixels` must be `width * height * 4` bytes of **non-premultiplied** RGBA.
/// Alpha is never modified.
///
/// `seed_alpha` is the minimum alpha for a pixel to be trusted as a colour
/// source (255 is the sane default); `fill_alpha` is the maximum alpha for a
/// pixel to be rewritten (0 repairs only fully transparent pixels — raise it to
/// also clean up semi-transparent ones).
#[wasm_bindgen]
pub fn repair(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    seed_alpha: u8,
    fill_alpha: u8,
) -> Result<Stats, JsError> {
    let opts = FillOptions {
        seed_alpha_min: seed_alpha,
        fill_alpha_max: fill_alpha,
    };

    let stats = fill_rgba(pixels, width, height, opts).map_err(|e| JsError::new(&e.to_string()))?;

    Ok(Stats {
        filled: stats.filled as u32,
        seeds: stats.seeds as u32,
        untouched: stats.untouched_partial as u32,
    })
}

/// The version of the underlying `prism-core`.
#[wasm_bindgen]
pub fn version() -> String {
    prism_core::VERSION.to_string()
}
