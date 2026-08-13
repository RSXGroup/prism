//! End-to-end tests that run the real `prism` binary against real PNG files.

use std::path::{Path, PathBuf};
use std::process::Command;

use image::{Rgba, RgbaImage};

/// The classic failure case: a solid sprite surrounded by transparent pixels
/// whose RGB an editor has crushed to black.
fn sprite_with_black_transparency(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, _| {
        if x < width / 2 {
            Rgba([255, 0, 0, 255]) // visible red
        } else {
            Rgba([0, 0, 0, 0]) // invisible, and quietly black
        }
    })
}

struct Workspace {
    dir: PathBuf,
}

impl Workspace {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("prism-it-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("cannot create the test workspace");
        Self { dir }
    }

    fn write_png(&self, name: &str, image: &RgbaImage) -> PathBuf {
        let path = self.dir.join(name);
        image.save(&path).expect("cannot write the fixture");
        path
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_prism"))
        .args(args)
        .output()
        .expect("cannot run the prism binary")
}

fn load(path: &Path) -> RgbaImage {
    image::open(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        .to_rgba8()
}

#[test]
fn repairs_a_png_and_leaves_alpha_alone() {
    let ws = Workspace::new("basic");
    let src = ws.write_png("sprite.png", &sprite_with_black_transparency(8, 8));

    let out = run(&[src.to_str().unwrap()]);
    assert!(out.status.success(), "prism failed: {:?}", out);

    let fixed = load(&ws.path("sprite.fixed.png"));
    assert_eq!(fixed.dimensions(), (8, 8));

    for (x, _y, px) in fixed.enumerate_pixels() {
        if x < 4 {
            assert_eq!(px.0, [255, 0, 0, 255], "visible pixels must not change");
        } else {
            assert_eq!(
                px.0,
                [255, 0, 0, 0],
                "hidden RGB should now be red, alpha still zero",
            );
        }
    }

    // The source must be untouched.
    let original = load(&src);
    assert_eq!(original.get_pixel(7, 0).0, [0, 0, 0, 0]);
}

#[test]
fn in_place_overwrites_the_source() {
    let ws = Workspace::new("inplace");
    let src = ws.write_png("sprite.png", &sprite_with_black_transparency(8, 8));

    let out = run(&["--in-place", src.to_str().unwrap()]);
    assert!(out.status.success(), "prism failed: {:?}", out);

    assert!(!ws.path("sprite.fixed.png").exists());
    assert_eq!(load(&src).get_pixel(7, 0).0, [255, 0, 0, 0]);
}

#[test]
fn dry_run_writes_nothing() {
    let ws = Workspace::new("dryrun");
    let src = ws.write_png("sprite.png", &sprite_with_black_transparency(8, 8));

    let out = run(&["--dry-run", src.to_str().unwrap()]);
    assert!(out.status.success(), "prism failed: {:?}", out);

    assert!(!ws.path("sprite.fixed.png").exists());
    assert_eq!(
        load(&src).get_pixel(7, 0).0,
        [0, 0, 0, 0],
        "source unchanged"
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("dry run"));
}

#[test]
fn a_clean_image_produces_no_output_file() {
    let ws = Workspace::new("clean");
    let opaque = RgbaImage::from_pixel(4, 4, Rgba([1, 2, 3, 255]));
    let src = ws.write_png("opaque.png", &opaque);

    let out = run(&[src.to_str().unwrap()]);
    assert!(out.status.success());
    assert!(!ws.path("opaque.fixed.png").exists());
    assert!(String::from_utf8_lossy(&out.stdout).contains("already clean"));
}

#[test]
fn scans_a_folder_and_skips_its_own_output() {
    let ws = Workspace::new("folder");
    ws.write_png("a.png", &sprite_with_black_transparency(6, 6));
    ws.write_png("b.png", &sprite_with_black_transparency(6, 6));
    // A leftover from a previous run — it must not be reprocessed.
    ws.write_png("c.fixed.png", &sprite_with_black_transparency(6, 6));

    let out = run(&[ws.dir.to_str().unwrap()]);
    assert!(out.status.success(), "prism failed: {:?}", out);

    assert!(ws.path("a.fixed.png").exists());
    assert!(ws.path("b.fixed.png").exists());
    assert!(
        !ws.path("c.fixed.fixed.png").exists(),
        "must not stack suffixes on re-runs",
    );
}

#[test]
fn out_dir_receives_the_results() {
    let ws = Workspace::new("outdir");
    let src = ws.write_png("sprite.png", &sprite_with_black_transparency(8, 8));
    let dest = ws.path("build");

    let out = run(&["--out-dir", dest.to_str().unwrap(), src.to_str().unwrap()]);
    assert!(out.status.success(), "prism failed: {:?}", out);
    assert!(dest.join("sprite.fixed.png").exists());
}

#[test]
fn rejects_thresholds_that_overlap() {
    let ws = Workspace::new("badargs");
    let src = ws.write_png("sprite.png", &sprite_with_black_transparency(4, 4));

    let out = run(&[
        "--seed-alpha",
        "10",
        "--fill-alpha",
        "10",
        src.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--seed-alpha"));
}

#[test]
fn raising_fill_alpha_repairs_semi_transparent_pixels_too() {
    let ws = Workspace::new("semi");
    // Left column opaque green, right column semi-transparent black.
    let image = RgbaImage::from_fn(2, 1, |x, _| {
        if x == 0 {
            Rgba([0, 255, 0, 255])
        } else {
            Rgba([0, 0, 0, 40])
        }
    });
    let src = ws.write_png("semi.png", &image);

    // Default thresholds leave the semi-transparent pixel alone.
    assert!(run(&[src.to_str().unwrap()]).status.success());
    assert!(!ws.path("semi.fixed.png").exists());

    // Raising --fill-alpha brings it into scope.
    let out = run(&["--fill-alpha", "128", src.to_str().unwrap()]);
    assert!(out.status.success(), "prism failed: {:?}", out);

    let fixed = load(&ws.path("semi.fixed.png"));
    assert_eq!(
        fixed.get_pixel(1, 0).0,
        [0, 255, 0, 40],
        "RGB repaired, alpha preserved",
    );
}
