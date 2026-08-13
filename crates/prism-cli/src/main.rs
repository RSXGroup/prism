//! prism — Probably Repairs Inconsistent Semi-transparency.

mod codec;
mod job;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, bail};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use prism_core::FillOptions;
use rayon::prelude::*;

use job::{Outcome, Plan};

#[derive(Parser, Debug)]
#[command(
    name = "prism",
    version,
    about = "Probably Repairs Inconsistent Semi-transparency",
    long_about = "Rewrites the invisible RGB data underneath transparent pixels with the \
                  colour of the nearest visible pixel, so bilinear filtering and mipmaps \
                  stop dragging a black halo out from under your sprites.\n\n\
                  Alpha is never modified. The image looks identical; it just samples correctly."
)]
struct Args {
    /// Images or folders to process.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,

    /// Descend into subfolders when a path is a directory.
    #[arg(short, long)]
    recursive: bool,

    /// Text inserted before the extension. Pass an empty string to drop it.
    #[arg(short, long, default_value = "fixed", value_name = "TEXT")]
    suffix: String,

    /// Write results into this directory instead of beside the sources.
    #[arg(short, long, value_name = "DIR")]
    out_dir: Option<PathBuf>,

    /// Overwrite the source files.
    #[arg(short, long, conflicts_with_all = ["suffix", "out_dir"])]
    in_place: bool,

    /// Files to process at once. Defaults to the number of CPU cores.
    #[arg(short, long, value_name = "N", value_parser = clap::value_parser!(u16).range(1..))]
    jobs: Option<u16>,

    /// Minimum alpha for a pixel to be trusted as a colour source.
    #[arg(long, default_value_t = 255, value_name = "0-255")]
    seed_alpha: u8,

    /// Maximum alpha for a pixel to have its colour rewritten.
    ///
    /// Raise this to also repair semi-transparent pixels, whose RGB editors
    /// tend to crush toward black.
    #[arg(long, default_value_t = 0, value_name = "0-255")]
    fill_alpha: u8,

    /// Report what would change without writing anything.
    #[arg(long)]
    dry_run: bool,

    /// Only print errors and the final summary.
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("prism: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<usize> {
    if args.paths.is_empty() {
        bail!("no input given — pass one or more image files or folders (try --help)");
    }
    if args.seed_alpha <= args.fill_alpha {
        bail!(
            "--seed-alpha ({}) must be greater than --fill-alpha ({}), \
             otherwise a pixel would be both a source and a target",
            args.seed_alpha,
            args.fill_alpha,
        );
    }

    let targets = collect_targets(&args)?;
    if targets.is_empty() {
        eprintln!(
            "prism: found no {} files to process",
            codec::SUPPORTED.join("/"),
        );
        return Ok(0);
    }

    if let Some(jobs) = args.jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs as usize)
            .build_global()
            .context("cannot configure the thread pool")?;
    }

    let opts = FillOptions {
        seed_alpha_min: args.seed_alpha,
        fill_alpha_max: args.fill_alpha,
    };
    let plan = Plan {
        suffix: &args.suffix,
        out_dir: args.out_dir.as_deref(),
        in_place: args.in_place,
        dry_run: args.dry_run,
        opts,
    };

    let bar = build_progress(targets.len(), args.quiet);
    let failed = AtomicUsize::new(0);
    let repaired = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let pixels = AtomicUsize::new(0);

    let mut lines: Vec<String> = targets
        .par_iter()
        .map(|path| {
            let outcome = job::process(path, &plan);
            bar.inc(1);

            let line = match &outcome {
                Ok(Outcome::Repaired { filled, output }) => {
                    repaired.fetch_add(1, Ordering::Relaxed);
                    pixels.fetch_add(*filled as usize, Ordering::Relaxed);
                    format!(
                        "  ok    {} -> {} ({} px)",
                        path.display(),
                        output.display(),
                        thousands(*filled),
                    )
                }
                Ok(Outcome::Clean) => {
                    skipped.fetch_add(1, Ordering::Relaxed);
                    format!("  clean {} (nothing to repair)", path.display())
                }
                Err(err) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                    format!("  FAIL  {}: {err:#}", path.display())
                }
            };
            (outcome.is_err(), line)
        })
        .filter(|(is_err, _)| *is_err || !args.quiet)
        .map(|(_, line)| line)
        .collect();

    bar.finish_and_clear();
    lines.sort_unstable();
    for line in &lines {
        println!("{line}");
    }

    let failed = failed.into_inner();
    println!(
        "\n{} file(s): {} repaired, {} already clean, {} failed{}",
        targets.len(),
        repaired.into_inner(),
        skipped.into_inner(),
        failed,
        if args.dry_run {
            "  [dry run — nothing written]"
        } else {
            ""
        },
    );
    let pixels = pixels.into_inner();
    if pixels > 0 {
        println!("{} pixels recoloured", thousands(pixels as u64));
    }

    Ok(failed)
}

/// Expands the given paths into a deduplicated, ordered list of image files.
fn collect_targets(args: &Args) -> Result<Vec<PathBuf>> {
    let mut targets = Vec::new();

    for path in &args.paths {
        if path.is_dir() {
            targets.extend(codec::scan_dir(path, args.recursive)?);
        } else if !path.exists() {
            bail!("{} does not exist", path.display());
        } else if codec::is_supported(path) {
            targets.push(path.clone());
        } else {
            eprintln!("prism: skipping {} (unsupported format)", path.display());
        }
    }

    // Re-running in suffix mode would otherwise produce `sprite.fixed.fixed.png`.
    if !args.in_place && !args.suffix.is_empty() {
        let marker = format!(".{}", args.suffix);
        targets.retain(|p| {
            let keep = !p
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.ends_with(&marker));
            if !keep {
                eprintln!(
                    "prism: skipping {} (already carries .{})",
                    p.display(),
                    args.suffix
                );
            }
            keep
        });
    }

    targets.sort_unstable();
    targets.dedup();
    Ok(targets)
}

fn build_progress(total: usize, quiet: bool) -> ProgressBar {
    if quiet || total <= 1 {
        return ProgressBar::hidden();
    }

    let bar = ProgressBar::new(total as u64);
    let style = ProgressStyle::with_template("  {bar:32} {pos}/{len}  {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("##-");
    bar.set_style(style);
    bar
}

/// `1234567` -> `1,234,567`, without pulling in a formatting crate.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(42), "42");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
