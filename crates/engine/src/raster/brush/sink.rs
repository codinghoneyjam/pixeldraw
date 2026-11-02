//! The span-shaped sink trait and the chunk-store implementation.

/// The `set` surface a stamp needs. [`crate::chunkstore::PixelWriter`] is the
/// production implementation; tests use a recorder.
///
/// **The run-shaped methods.** `set`/`get` are one pixel at a time and every
/// producer of pixels here actually produces *spans*: a round pen is one run per
/// row, a scanline fill is one run per seed, a polygon edge is one run per row.
/// Writing a span through `set` costs a ledger update and a map probe per pixel,
/// which is what made the pen-64 stroke take seconds. So the trait carries the
/// span shapes, and every one of them has a **default body that is the per-pixel
/// loop** — a recording sink that implements only `set`/`get` therefore still
/// sees exactly the cells it saw before, in the same order, and every existing
/// test and golden keeps its meaning. [`crate::chunkstore::PixelWriter`]
/// overrides them with chunk-resolved versions.
pub trait PixelSink {
    fn set(&mut self, x: i32, y: i32, packed: u32);
    fn get(&self, x: i32, y: i32) -> u32;

    /// `len` pixels of one colour starting at `(x, y)`.
    fn set_run(&mut self, x: i32, y: i32, len: usize, color: &[u8; 4]) {
        let packed = u32::from_le_bytes(*color);
        for i in 0..len as i32 {
            self.set(x + i, y, packed);
        }
    }

    /// Paint over the maximal run of `seed` on row `y` that contains `x`,
    /// and report it as `(left, right)` **inclusive**.
    ///
    /// `w`/`h` bound the walk exactly as `flood_fill_scanline` did. `None` means
    /// `(x, y)` is out of bounds or is not `seed`, i.e. there is no span here.
    fn fill_span(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        seed: u32,
        fill: u32,
    ) -> Option<(i32, i32)> {
        if y < 0 || y >= h || x < 0 || x >= w || self.get(x, y) != seed {
            return None;
        }
        let mut l = x;
        while l > 0 && self.get(l - 1, y) == seed {
            l -= 1;
        }
        let mut r = x;
        while r + 1 < w && self.get(r + 1, y) == seed {
            r += 1;
        }
        self.set_run(l, y, (r - l + 1) as usize, &fill.to_le_bytes());
        Some((l, r))
    }

    /// Append every run of `seed` inside `[lo, hi)` of row `y`, left to right.
    fn seed_runs(&self, y: i32, lo: i32, hi: i32, seed: u32, out: &mut Vec<(i32, i32)>) {
        let mut i = lo;
        while i < hi {
            if self.get(i, y) == seed {
                let s = i;
                while i < hi && self.get(i, y) == seed {
                    i += 1;
                }
                out.push((s, i));
            } else {
                i += 1;
            }
        }
    }
}

impl PixelSink for crate::chunkstore::PixelWriter<'_> {
    #[inline]
    fn set(&mut self, x: i32, y: i32, packed: u32) {
        // Out-of-canvas is dropped here rather than wrapping: the JS tree's
        // `stampBrush` relied on `writer.set` ignoring negatives, and a bare
        if x < 0 || y < 0 {
            return;
        }
        crate::chunkstore::PixelWriter::set(self, x as u32, y as u32, packed);
    }

    #[inline]
    fn get(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 {
            return 0;
        }
        crate::chunkstore::PixelWriter::get(self, x as u32, y as u32)
    }

    /// All clipping — including "trim a run that starts off-canvas" — lives in
    /// [`crate::chunkstore::PixelWriter::set_run`], so there is one rule, not two.
    #[inline]
    fn set_run(&mut self, x: i32, y: i32, len: usize, color: &[u8; 4]) {
        crate::chunkstore::PixelWriter::set_run(self, x, y, len, color);
    }

    /// Chunk-stepped: the span edges come from [`PixelWriter::row_seed_runs`],
    /// and the fill is one `set_run` instead of `r - l + 1` writes.
    fn fill_span(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        seed: u32,
        fill: u32,
    ) -> Option<(i32, i32)> {
        if y < 0 || y >= h || x < 0 || x >= w || self.pixel_at(x, y) != seed {
            return None;
        }
        let mut buf = Vec::new();
        // Left edge: the run in [0, x+1) that actually contains x.
        self.row_seed_runs(y, 0, x + 1, seed, &mut buf);
        let l = buf.last()?.0;
        // Right edge: the first run at or after x, extended to the row end.
        buf.clear();
        self.row_seed_runs(y, x, w, seed, &mut buf);
        let r = buf.first()?.1 - 1;
        crate::chunkstore::PixelWriter::set_run(
            self,
            l,
            y,
            (r - l + 1) as usize,
            &fill.to_le_bytes(),
        );
        Some((l, r))
    }

    fn seed_runs(&self, y: i32, lo: i32, hi: i32, seed: u32, out: &mut Vec<(i32, i32)>) {
        crate::chunkstore::PixelWriter::row_seed_runs(self, y, lo, hi, seed, out);
    }
}
