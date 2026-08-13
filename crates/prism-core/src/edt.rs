//! Exact Euclidean feature transform.
//!
//! Given a boolean mask of "site" pixels, this computes for *every* pixel the
//! index of its nearest site under the true Euclidean metric, in O(width ×
//! height) regardless of how many sites there are.
//!
//! The method is the two-phase decomposition from Felzenszwalb & Huttenlocher,
//! *Distance Transforms of Sampled Functions* (2012), extended to carry the
//! argmin so we recover the nearest site itself rather than just its distance:
//!
//! 1. **Column pass.** For each column, find the nearest site within that
//!    column. Because the seed function is `{0, ∞}` this collapses to two
//!    linear scans (down, then up) instead of a full envelope construction.
//! 2. **Row pass.** For each row, take the column-pass distances as a sampled
//!    1-D function and compute the lower envelope of the parabolas
//!    `g(x') + (x - x')²`. The winning `x'` names the column of the nearest
//!    site; the column pass already told us its row.
//!
//! This is exact — not an approximation like jump flooding — and it replaces
//! both the seed-collection step and the Delaunay query of the original
//! implementation.

/// Sentinel distance standing in for "no site in this column". Kept well below
/// `i64::MAX` so `g + x²` cannot overflow.
const INF: i64 = i64::MAX / 4;

/// Returned for a pixel when the image contains no sites at all.
pub const NO_SITE: u32 = u32::MAX;

/// For every pixel, the flat index (`y * width + x`) of the nearest site pixel.
///
/// Returns [`NO_SITE`] in every slot if `is_site` contains no `true` values.
///
/// # Panics
/// Panics if `is_site.len() != width * height`.
pub fn feature_transform(is_site: &[bool], width: usize, height: usize) -> Vec<u32> {
    assert_eq!(
        is_site.len(),
        width * height,
        "site mask length does not match the given dimensions",
    );

    let mut nearest = vec![NO_SITE; width * height];
    if width == 0 || height == 0 {
        return nearest;
    }

    // ── Phase 1: nearest site within each column ─────────────────────────────
    // `col_dist` is the squared vertical distance, `col_y` the site's row.
    let mut col_dist = vec![INF; width * height];
    let mut col_y = vec![0u32; width * height];

    for x in 0..width {
        // Sweep downwards: carry the most recent site seen above.
        let mut last: i64 = -1;
        for y in 0..height {
            let i = y * width + x;
            if is_site[i] {
                last = y as i64;
                col_dist[i] = 0;
                col_y[i] = y as u32;
            } else if last >= 0 {
                let d = y as i64 - last;
                col_dist[i] = d * d;
                col_y[i] = last as u32;
            }
        }

        // Sweep upwards: keep whichever direction turned out closer.
        let mut last: i64 = -1;
        for y in (0..height).rev() {
            let i = y * width + x;
            if is_site[i] {
                last = y as i64;
            } else if last >= 0 {
                let d = last - y as i64;
                let dd = d * d;
                if dd < col_dist[i] {
                    col_dist[i] = dd;
                    col_y[i] = last as u32;
                }
            }
        }
    }

    // ── Phase 2: lower envelope across each row ──────────────────────────────
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;

        nearest.par_chunks_mut(width).enumerate().for_each_init(
            || (Vec::<usize>::new(), Vec::<f64>::new()),
            |(v, z), (y, out_row)| {
                let row = y * width;
                envelope_row(
                    &col_dist[row..row + width],
                    &col_y[row..row + width],
                    out_row,
                    width,
                    v,
                    z,
                );
            },
        );
    }

    #[cfg(not(feature = "parallel"))]
    {
        let mut v: Vec<usize> = Vec::with_capacity(width);
        let mut z: Vec<f64> = Vec::with_capacity(width + 1);

        for (y, out_row) in nearest.chunks_mut(width).enumerate() {
            let row = y * width;
            envelope_row(
                &col_dist[row..row + width],
                &col_y[row..row + width],
                out_row,
                width,
                &mut v,
                &mut z,
            );
        }
    }

    nearest
}

/// Resolves one row of the feature transform.
///
/// `g` holds the squared column-pass distances for this row and `sy` the
/// corresponding site rows. `v` and `z` are reusable scratch buffers: `v[k]` is
/// the column owning the k-th envelope segment and `z[k]` is where that segment
/// begins.
fn envelope_row(
    g: &[i64],
    sy: &[u32],
    out: &mut [u32],
    width: usize,
    v: &mut Vec<usize>,
    z: &mut Vec<f64>,
) {
    v.clear();
    z.clear();

    for q in 0..width {
        // Columns with no site anywhere contribute no parabola.
        if g[q] >= INF {
            continue;
        }
        let fq = g[q] + (q * q) as i64;

        // Pop segments that this parabola completely covers.
        loop {
            let Some(&p) = v.last() else {
                z.push(f64::NEG_INFINITY);
                break;
            };
            let fp = g[p] + (p * p) as i64;
            let s = (fq - fp) as f64 / (2.0 * (q as f64 - p as f64));
            if s <= *z.last().expect("z and v are kept the same length") {
                v.pop();
                z.pop();
            } else {
                z.push(s);
                break;
            }
        }
        v.push(q);
    }

    // No parabolas at all means the whole image is siteless.
    if v.is_empty() {
        out.fill(NO_SITE);
        return;
    }

    let mut k = 0usize;
    for (q, slot) in out.iter_mut().enumerate() {
        while k + 1 < v.len() && z[k + 1] < q as f64 {
            k += 1;
        }
        let nx = v[k];
        *slot = sy[nx] * width as u32 + nx as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O(n²) ground truth: scan every site for every pixel.
    fn brute_force(is_site: &[bool], width: usize, height: usize) -> Vec<Option<(usize, usize)>> {
        let sites: Vec<(usize, usize)> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .filter(|&(x, y)| is_site[y * width + x])
            .collect();

        (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| {
                sites.iter().copied().min_by_key(|&(sx, sy)| {
                    let dx = sx as i64 - x as i64;
                    let dy = sy as i64 - y as i64;
                    dx * dx + dy * dy
                })
            })
            .collect()
    }

    fn sq_dist(a: (usize, usize), b: (usize, usize)) -> i64 {
        let dx = a.0 as i64 - b.0 as i64;
        let dy = a.1 as i64 - b.1 as i64;
        dx * dx + dy * dy
    }

    /// Deterministic xorshift so the test is reproducible without a dep.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn empty_mask_yields_no_sites() {
        let ft = feature_transform(&[false; 16], 4, 4);
        assert!(ft.iter().all(|&s| s == NO_SITE));
    }

    #[test]
    fn a_site_is_its_own_nearest() {
        let mut mask = [false; 25];
        mask[12] = true; // centre of a 5×5
        let ft = feature_transform(&mask, 5, 5);
        assert_eq!(ft[12], 12);
        // Corner (0,0) has only one site to pick.
        assert_eq!(ft[0], 12);
    }

    #[test]
    fn matches_brute_force_on_random_masks() {
        let mut rng = Rng(0x9E3779B97F4A7C15);

        for &(w, h) in &[(1, 1), (1, 9), (9, 1), (7, 5), (16, 16), (23, 17)] {
            for density in [2u64, 8, 32] {
                let mask: Vec<bool> = (0..w * h).map(|_| rng.next() % density == 0).collect();
                if !mask.iter().any(|&b| b) {
                    continue;
                }

                let got = feature_transform(&mask, w, h);
                let want = brute_force(&mask, w, h);

                for y in 0..h {
                    for x in 0..w {
                        let i = y * w + x;
                        let expected = want[i].expect("mask has at least one site");
                        let g = got[i];
                        assert_ne!(g, NO_SITE, "{w}x{h} d={density} at ({x},{y})");
                        let actual = (g as usize % w, g as usize / w);

                        assert!(
                            mask[actual.1 * w + actual.0],
                            "{w}x{h} d={density}: ({x},{y}) resolved to non-site {actual:?}",
                        );
                        // Ties are legitimate, so compare distances not indices.
                        assert_eq!(
                            sq_dist(actual, (x, y)),
                            sq_dist(expected, (x, y)),
                            "{w}x{h} d={density}: ({x},{y}) got {actual:?}, want {expected:?}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn resolves_exact_diagonal_ties_consistently() {
        // Two sites equidistant from the centre column.
        let mut mask = [false; 9];
        mask[0] = true; // (0,0)
        mask[2] = true; // (2,0)
        let ft = feature_transform(&mask, 3, 3);
        // (1,0) is 1 away from both; either is correct, neither may be missing.
        assert!(ft[1] == 0 || ft[1] == 2);
    }
}
