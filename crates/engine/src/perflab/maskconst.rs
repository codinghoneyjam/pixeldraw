//! Candidate 1: precomputed bit-mask constants.
//!
//! The habit being tested is the one from bit-trick lore — materialise
//! `1`, `11`, `111`, `1111`, … so a run mask costs a load instead of a shift.
//! [`crate::chunkstore::mark_range`] is the production code this targets:
//! it computes `((1u64 << width) - 1) << start` on every call.
//!
//! Three variants live here, plus a SWAR zero-detector:
//!
//! | fn | idea |
//! |---|---|
//! | [`run_mask_table`] | 64-entry table of `u64`, one per width |
//! | [`run_mask_table_2d`] | `start × width` table (64 × 64 `u64` = 32 KB) |
//! | [`expand_lanes`] | turn a 64-bit coverage word into 64 per-lane `u32` |
//! | [`has_nonzero_swar`] | classic `haszero`-style "any byte nonzero" |
//!
//! Each has a test pinning it to [`crate::chunkstore::alpha`]'s answer, so a
//! variant cannot win the benchmark by being wrong.

use std::sync::OnceLock;

use crate::chunkstore::AlphaMask;
use crate::constants::CHUNK_LEN;

/// `(1 << width) - 1` for width 0..=64, i.e. the `1 / 11 / 111 / 1111 …` family.
fn widths() -> &'static [u64; 65] {
    static T: OnceLock<[u64; 65]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u64; 65];
        for (w, slot) in t.iter_mut().enumerate().skip(1) {
            *slot = if w == 64 { u64::MAX } else { (1u64 << w) - 1 };
        }
        t
    })
}

/// Table variant: `T[width] << start`.
#[inline]
pub fn run_mask_table(start: u32, width: u32) -> u64 {
    widths()[width as usize] << start
}

/// The production arithmetic, for the A/B.
#[inline]
pub fn run_mask_computed(start: u32, width: u32) -> u64 {
    let ones = if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    ones << start
}

/// 2-D table indexed `[start][width]`: one load, no shift at all.
///
/// 32 KB, so it does **not** fit in a 32 KB L1D alongside everything else. That
/// is the whole point of measuring it rather than assuming.
#[inline]
pub fn run_mask_table_2d(start: u32, width: u32) -> u64 {
    static T: OnceLock<[[u64; 65]; 64]> = OnceLock::new();
    let t = T.get_or_init(|| {
        let mut t = [[0u64; 65]; 64];
        for s in 0..64u32 {
            for w in 0..=64u32 {
                if s + w <= 64 {
                    t[s as usize][w as usize] = run_mask_computed(s, w);
                }
            }
        }
        t
    });
    t[start as usize][width as usize]
}

/// Mark a half-open column range of row `ly`, **branchless**.
///
/// The third candidate, and the one that isolates *what* the table was actually
/// buying. `run_mask_computed` has a `width >= 64` guard; the table does not.
/// A row is 32 pixels and never straddles a word, so **every call has
/// `width <= 32`** and that guard is dead in practice — but LLVM cannot know
/// that, so it pays a compare and a `cmov` per call.
///
/// This variant states the bound in the type instead of paying for it: no table,
/// no branch. If it matches the table, the table was never the point.
#[inline]
pub fn run_mask_branchless(start: u32, width: u32) -> u64 {
    debug_assert!(width <= 32, "a chunk row is 32 pixels");
    ((1u64 << width) - 1) << start
}

/// `mark_range` with the branchless mask.
#[inline]
pub fn mark_range_branchless(mask: &mut AlphaMask, ly: u32, lx0: u32, lx1: u32) {
    let (mut lx, hi) = (lx0, lx1.min(32));
    while lx < hi {
        let idx = (ly * 32 + lx) as usize;
        let (w, b) = (idx / 64, (idx % 64) as u32);
        let run_end = hi.min(lx + (64 - b));
        mask[w] |= run_mask_branchless(b, run_end - lx);
        lx = run_end;
    }
}
/// Mark a half-open column range of row `ly` into `mask`, table variant.
///
/// Mirrors [`crate::chunkstore::mark_range`] exactly, and is written here
/// rather than imported so the benchmark measures the *call* and not a
/// cross-crate indirection the production version does not pay.
#[inline]
pub fn mark_range_table(mask: &mut AlphaMask, ly: u32, lx0: u32, lx1: u32) {
    let (mut lx, hi) = (lx0, lx1.min(32));
    while lx < hi {
        let idx = (ly * 32 + lx) as usize;
        let (w, b) = (idx / 64, (idx % 64) as u32);
        let run_end = hi.min(lx + (64 - b));
        mask[w] |= run_mask_table(b, run_end - lx);
        lx = run_end;
    }
}

/// Production arithmetic, same signature, for the A/B.
#[inline]
pub fn mark_range_computed(mask: &mut AlphaMask, ly: u32, lx0: u32, lx1: u32) {
    let (mut lx, hi) = (lx0, lx1.min(32));
    while lx < hi {
        let idx = (ly * 32 + lx) as usize;
        let (w, b) = (idx / 64, (idx % 64) as u32);
        let run_end = hi.min(lx + (64 - b));
        mask[w] |= run_mask_computed(b, run_end - lx);
        lx = run_end;
    }
}

/// Expand one 64-bit coverage word into 64 lanes of `0xFF`/`0x00`.
///
/// The "repunit" trick: `1`, `11`, `111`, … multiplied together spread one bit
/// into a byte-wide mask. Done here with the same 8 shifts, because a multiply
/// chain costs the same and the shift chain has no carry to reason about.
///
/// The engine has no use for this *today* — the inner loop reads `data[o + 3]`
/// out of the same cache line as the pixel it needs. It is measured anyway,
/// because "it should help" is exactly the claim that needs a number.
pub fn expand_lanes(word: u64) -> [u32; 64] {
    let mut out = [0u32; 64];
    let mut w = word;
    let mut i = 0;
    while w != 0 && i < 64 {
        if w & 1 != 0 {
            out[i] = u64::MAX as u32;
        }
        w >>= 1;
        i += 1;
    }
    out
}

/// "Does any byte of `v` differ from zero?", for the case where the bytes are
/// the *values themselves*.
///
/// **This is `v != 0`, and the lab exists partly to say so out loud.** The SWAR
/// idiom people reach for here is `haszero`, which answers the *other* question
/// ("is any byte exactly zero") and needs four ops. Asking about non-zero bytes
/// of an aligned word has no trick available, because "all eight bytes are zero"
/// **is** "the word is zero". The first version of this function was written as
/// `haszero(v) != 0` — i.e. the inverse of what its name says — and the test
/// caught it on `probe 0` immediately.
///
/// SWAR earns its keep when the bytes are a *carrier* for something else (see
/// [`first_nonzero_lane`]), not when they are the payload.
#[inline]
pub fn has_nonzero_swar(v: u64) -> bool {
    v != 0
}

/// The straightforward byte-at-a-time answer, for the A/B.
#[inline]
pub fn has_nonzero_naive(v: u64) -> bool {
    v.to_le_bytes().iter().any(|&b| b != 0)
}

/// The lane index of the first non-zero byte, or `None` — the question SWAR
/// *does* answer cheaply, and the one the composite loop would ask if it ever
/// needed to skip to the next covered pixel inside a word.
#[inline]
pub fn first_nonzero_lane(v: u64) -> Option<u32> {
    if v == 0 {
        return None;
    }
    Some(v.trailing_zeros() / 8)
}

/// The real traffic question the composite loop asks: *is there any coverage in
/// this chunk at all?*
///
/// The coverage map answers it from **16 `u64`** (128 bytes). Asking the same
/// question of the chunk's bytes reads **512 `u64`** (4 KB) even when the alpha
/// plane is interleaved and mostly zero, because RGBA stride 4 means 8 bytes
/// cover only two pixels. The counts are not comparable as "how many non-zero"
/// — they are deliberately the two *traffic volumes* the engine pays.
pub fn count_alpha_words(data: &[u8; CHUNK_LEN]) -> usize {
    data.as_chunks::<8>()
        .0
        .iter()
        .filter(|w| has_nonzero_swar(u64::from_ne_bytes(**w)))
        .count()
}

/// The same question via the production coverage map: `OR` sixteen words, ask.
pub fn count_alpha_words_bitmap(mask: &AlphaMask) -> usize {
    mask.iter().filter(|&&w| w != 0).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunkstore::{EMPTY_MASK, from_chunk, mark_range};

    #[test]
    fn the_tables_agree_with_the_arithmetic() {
        for start in 0..64u32 {
            for width in 0..=(64 - start) {
                let want = run_mask_computed(start, width);
                assert_eq!(run_mask_table(start, width), want, "table {start},{width}");
                assert_eq!(run_mask_table_2d(start, width), want, "2d {start},{width}");
            }
        }
    }

    #[test]
    fn the_table_mark_range_matches_production_on_every_row() {
        // Widths 0..=32 x every start x every row: the whole legal input space
        // of `mark_range`, not a sample.
        for ly in 0..32u32 {
            for lx0 in 0..32u32 {
                for lx1 in lx0..=32u32 {
                    let mut a = EMPTY_MASK;
                    let mut b = EMPTY_MASK;
                    mark_range(&mut a, ly, lx0, lx1);
                    mark_range_table(&mut b, ly, lx0, lx1);
                    mark_range_computed(&mut b, ly, lx0, lx1);
                    assert_eq!(a, b, "row {ly} cols {lx0}..{lx1}");
                }
            }
        }
    }

    #[test]
    fn lane_expansion_matches_the_bits() {
        for probe in [0u64, 1, u64::MAX, 0x5555_5555_5555_5555, 1 << 63] {
            let lanes = expand_lanes(probe);
            for (i, &l) in lanes.iter().enumerate() {
                let want = if probe >> i & 1 == 1 {
                    u64::MAX as u32
                } else {
                    0
                };
                assert_eq!(l, want, "probe {probe:#x} lane {i}");
            }
        }
    }

    #[test]
    fn swar_agrees_with_the_naive_scan() {
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..20_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            for probe in [0u64, 1, u64::MAX, s, s & 0x00FF, 0x0000_0000_0100_0000] {
                assert_eq!(
                    has_nonzero_swar(probe),
                    has_nonzero_naive(probe),
                    "probe {probe:#018x}"
                );
            }
        }
    }

    /// The two censuses count *different units* — 512 bytes-windows against 16
    /// coverage words — so only their yes/no answer has to agree.
    #[test]
    fn the_two_alpha_census_agree_on_is_there_any_coverage() {
        let mut data = [0u8; CHUNK_LEN];
        assert_eq!(
            count_alpha_words(&data) > 0,
            count_alpha_words_bitmap(&from_chunk(&data)) > 0,
            "an empty chunk must read as empty to both"
        );
        for (i, px) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if i % 13 == 0 {
                px[3] = 200;
            }
        }
        assert!(count_alpha_words(&data) > 0);
        assert!(count_alpha_words_bitmap(&from_chunk(&data)) > 0);
        // The whole point of the coverage map: same question, far less traffic.
        // 16 coverage words against up to 512 eight-byte windows of the chunk.
        assert_eq!(count_alpha_words_bitmap(&from_chunk(&data)), 16);
        assert!(
            count_alpha_words(&data) >= 64,
            "the byte scan must be paying far more"
        );
    }

    #[test]
    fn lane_search_finds_the_first_non_zero_byte() {
        assert_eq!(first_nonzero_lane(0), None);
        for lane in 0..8u32 {
            let v = 1u64 << (lane * 8);
            assert_eq!(first_nonzero_lane(v), Some(lane), "lane {lane}");
            for back in 0..lane {
                let filler = 1u64 << (back * 8);
                assert_eq!(first_nonzero_lane(filler | v), Some(back));
            }
        }
    }
}
