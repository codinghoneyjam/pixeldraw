//! Size-guarded footprint cache and the row-run derivation.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::constants::{PEN_MAX, PEN_MIN};
use crate::errors::{DrawToolError, EngineResult};

/// A cached circular footprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrushFootprint {
    pub n: usize,
    /// `(n - 1) >> 1`, the offset from the stamp centre to the mask's origin.
    pub offset: usize,
    /// Row-major `n * n`, 1 = painted.
    pub mask: Vec<u8>,
    /// Per row, the horizontal runs of painted cells as `(start, len)`.
    ///
    /// The disc rule makes every row one contiguous run, so this is `n` runs for
    /// the whole stamp — a pen-64 footprint goes from 3,228 `set` calls to 64
    /// [`crate::raster::brush::PixelSink::set_run`] calls, and the mask bytes are
    /// then never touched again after the first build. Derived from `mask`,
    /// never a second source of truth: `brush_footprint_runs_match_mask` pins
    /// them together.
    pub runs: Vec<Vec<(usize, usize)>>,
}

impl BrushFootprint {
    #[inline]
    pub fn is_set(&self, x: usize, y: usize) -> bool {
        self.mask[y * self.n + x] == 1
    }
}

fn cache() -> &'static std::sync::Mutex<HashMap<usize, BrushFootprint>> {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<usize, BrushFootprint>>> = OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Size-guarded, cached footprint for pen diameter `n`.
pub fn brush_footprint(n: usize) -> EngineResult<BrushFootprint> {
    if !(PEN_MIN as usize..=PEN_MAX as usize).contains(&n) {
        return Err(DrawToolError::out_of_range(format!(
            "pen size {n} out of range"
        )));
    }
    let mut guard = cache().lock().expect("footprint cache poisoned");
    if let Some(fp) = guard.get(&n) {
        return Ok(fp.clone());
    }
    let mut mask = vec![0u8; n * n];
    let nn = (n * n) as i64;
    for y in 0..n as i64 {
        for x in 0..n as i64 {
            let lhs = (2 * x + 1 - n as i64).pow(2) + (2 * y + 1 - n as i64).pow(2);
            if lhs <= nn {
                mask[(y as usize) * n + x as usize] = 1;
            }
        }
    }
    let fp = BrushFootprint {
        n,
        offset: (n - 1) >> 1,
        runs: row_runs(&mask, n),
        mask,
    };
    guard.insert(n, fp.clone());
    Ok(fp)
}

/// The horizontal runs of painted cells, one `Vec` per row.
///
/// Scanning left to right and emitting a run at each transition keeps this in
/// step with `mask` by construction, whatever the footprint turns out to be —
/// an n=1 or n=2 pen has rows that are not contiguous in general.
fn row_runs(mask: &[u8], n: usize) -> Vec<Vec<(usize, usize)>> {
    let mut runs = Vec::with_capacity(n);
    for y in 0..n {
        let row = &mask[y * n..(y + 1) * n];
        let mut out = Vec::new();
        let mut x = 0;
        while x < n {
            if row[x] == 1 {
                let start = x;
                while x < n && row[x] == 1 {
                    x += 1;
                }
                out.push((start, x - start));
            } else {
                x += 1;
            }
        }
        runs.push(out);
    }
    runs
}
