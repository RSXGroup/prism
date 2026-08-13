//! Generates a synthetic test image exhibiting the bug prism repairs.
//!
//! Draws a few overlapping circles of flat colour and crushes the RGB of every
//! transparent pixel to black, which is what most editors write out.
//!
//! ```text
//! cargo run --release --example make_fixture -- fixture.png 2048
//! ```

use image::{Rgba, RgbaImage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "fixture.png".into());
    let size: u32 = args
        .next()
        .unwrap_or_else(|| "1024".into())
        .parse()
        .map_err(|_| "size must be a positive integer")?;

    let blobs = [
        (0.35, 0.35, 0.22, [220, 40, 40u8]),
        (0.62, 0.45, 0.18, [40, 200, 90u8]),
        (0.45, 0.68, 0.20, [60, 110, 235u8]),
    ];

    let image = RgbaImage::from_fn(size, size, |x, y| {
        let (fx, fy) = (x as f64 / size as f64, y as f64 / size as f64);

        for &(cx, cy, r, rgb) in &blobs {
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            if d <= r {
                return Rgba([rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        // Transparent — and, as an editor would leave it, black underneath.
        Rgba([0, 0, 0, 0])
    });

    image.save(&path)?;

    let transparent = image.pixels().filter(|p| p.0[3] == 0).count();
    println!(
        "wrote {path} ({size}x{size}, {transparent} transparent px of {})",
        size as usize * size as usize,
    );
    Ok(())
}
