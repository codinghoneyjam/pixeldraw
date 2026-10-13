//! One bit per chunk pixel: *"this pixel has a non-zero alpha"*.
//!
//! **Why this exists.** Compositing and flattening both answer the same
//! question millions of times per frame: *is any layer opaque here at all?*
//! The answer used to be found by loading `data[o + 3]` for every
//! (pixel, layer) pair — 1.4e8 single-byte loads for one flatten of a 64-layer
//! document, of which 95.5% are known-zero in advance. A 1024-bit mask is
//! **128 bytes instead of 1024**, so 64 layers cost 8 KB of mask reads rather
//! than 4 KB×64 of pixel reads, and a whole 64-pixel word that no layer touches
//! is rejected with one `u64` test instead of 64×64 branch checks.
//!
//! This is the "coverage map" every 2D rasteriser carries (Skia's `kAlpha_8`
//! masks, pixman's RLE sparse regions), expressed in safe Rust with no
//! architecture-specific code: the words are plain `u64`s, so LLVM lowers them
//! to whatever the target has, and the *same* binary is correct everywhere.
//!
//! Bit layout is row-major over the chunk: bit `y * 32 + x` lives in word
//! `(y * 32 + x) / 64` at position `(y * 32 + x) % 64`. Bits above the chunk's
//! 1024 pixels cannot exist, so `OR` and `AND` over the words are exact and
//! never need masking off a tail.

use crate::constants::CHUNK_PX;

/// Words per chunk mask: 32×32 pixels at 64 pixels per word.
pub const MASK_WORDS: usize = CHUNK_PX as usize * CHUNK_PX as usize / 64;

/// The per-chunk coverage map. Index `y * 32 + x` maps to bit `idx`.
///
/// `AlphaMask` is `Copy`, so a caller can keep the "union so far" in a
/// register-ish local and AND the next layer's mask into it.
pub type AlphaMask = [u64; MASK_WORDS];

/// The mask of a chunk with nothing in it.
pub const EMPTY_MASK: AlphaMask = [0u64; MASK_WORDS];

/// Word/position of the pixel `(lx, ly)` inside its chunk.
#[inline]
const fn locate(lx: u32, ly: u32) -> (usize, u32) {
    let idx = (ly * CHUNK_PX + lx) as usize;
    (idx / 64, (idx % 64) as u32)
}

/// Mark one pixel covered.
#[inline]
pub fn mark(mask: &mut AlphaMask, lx: u32, ly: u32) {
    let (w, b) = locate(lx, ly);
    mask[w] |= 1u64 << b;
}

/// Unmark one pixel.
#[inline]
pub fn unmark(mask: &mut AlphaMask, lx: u32, ly: u32) {
    let (w, b) = locate(lx, ly);
    mask[w] &= !(1u64 << b);
}

/// Mark the half-open column range `lx0..lx1` of row `ly`.
#[inline]
pub fn mark_range(mask: &mut AlphaMask, ly: u32, lx0: u32, lx1: u32) {
    let (mut lx, hi) = (lx0, lx1.min(CHUNK_PX));
    while lx < hi {
        // `locate` gives the word and the bit offset **within that word**, so the
        // run ends at the next word boundary measured from `lx` — not at a fixed
        // column. Rows are 32 pixels wide, so words straddle row seams and a
        // fixed boundary would write bits into the neighbouring row.
        let (w, b) = locate(lx, ly);
        let word_end = lx + (64 - b);
        let run_end = hi.min(word_end);
        let width = run_end - lx;
        mask[w] |= mask_from_range(b, width);
        lx = run_end;
    }
}

/// Low `width` ones shifted left by `start`, for [`mark_range`].
///
/// `# Panics` (debug): `width > 64`.
#[inline]
fn mask_from_range(start: u32, width: u32) -> u64 {
    debug_assert!(width <= 64);
    let ones = if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    ones << start
}

/// Rebuild a mask from chunk bytes.
///
/// This is the only place that reads a whole chunk's alpha, and it runs once per
/// stored chunk — on `put_chunk`, not on every composite.
pub fn from_chunk(data: &[u8]) -> AlphaMask {
    let mut mask = EMPTY_MASK;
    for (idx, px) in data.as_chunks::<4>().0.iter().enumerate() {
        if px[3] != 0 {
            mask[idx / 64] |= 1u64 << (idx % 64);
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::CHUNK_LEN;

    #[test]
    fn words_cover_the_whole_chunk() {
        assert_eq!(MASK_WORDS * 64, CHUNK_PX as usize * CHUNK_PX as usize);
        assert_eq!(CHUNK_LEN / 4, MASK_WORDS * 64);
    }

    #[test]
    fn mark_and_unmark_round_trip_at_the_far_corner() {
        let mut m = EMPTY_MASK;
        mark(&mut m, 31, 31);
        // Pixel 1023 is the last bit of the last word.
        assert_eq!(m[MASK_WORDS - 1], 1u64 << 63);
        assert_ne!(m, EMPTY_MASK);
        unmark(&mut m, 31, 31);
        assert_eq!(m, EMPTY_MASK);
    }

    /// A run cannot straddle two words *within* a row — a row is 32 pixels and a
    /// word is 64 — but it has to land in the right word for **every** row, because
    /// rows pair up two per word: row 0 and row 1 share word 0, rows 2 and 3 share
    /// word 1, and so on. A helper that assumed a fixed column boundary would spill
    /// row 1's pixels into row 0's half of the word.
    #[test]
    fn a_full_row_lands_in_the_right_word() {
        for ly in [0u32, 1, 2, 3, 30, 31] {
            let mut m = EMPTY_MASK;
            mark_range(&mut m, ly, 0, 32);
            let mut c = vec![0u8; CHUNK_LEN];
            for x in 0..32 {
                c[((ly * 32 + x) as usize) * 4 + 3] = 255;
            }
            assert_eq!(m, from_chunk(&c), "row {ly}");
        }
    }

    /// Splitting a row into several runs must give the same word as one run.
    #[test]
    fn split_runs_agree_with_one_run() {
        for ly in [0u32, 1, 5, 31] {
            let mut one = EMPTY_MASK;
            mark_range(&mut one, ly, 3, 29);
            let mut three = EMPTY_MASK;
            mark_range(&mut three, ly, 3, 7);
            mark_range(&mut three, ly, 7, 20);
            mark_range(&mut three, ly, 20, 29);
            assert_eq!(one, three, "row {ly}");
        }
    }

    #[test]
    fn from_chunk_agrees_with_the_byte_scan() {
        let mut c = vec![0u8; CHUNK_LEN];
        for (i, px) in c.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if i % 7 == 0 {
                px[3] = 1; // alpha != 0 is all that matters
            }
        }
        let mask = from_chunk(&c);
        let mut want = EMPTY_MASK;
        for i in 0..1024 {
            if i % 7 == 0 {
                want[i / 64] |= 1u64 << (i % 64);
            }
        }
        assert_eq!(mask, want);
    }
}
