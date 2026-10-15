//! Module 2 — head-to-head lab. Run:
//!
//! ```text
//! cargo run -p pixel-engine --release --example perflab
//! ```
//!
//! Two candidates, measured against what the engine does **today**. Nothing here
//! is wired into the engine; see [`pixel_engine::perflab`].
//!
//! * **1 — mask constants.** Is the `1 / 11 / 111` table habit worth anything at
//!   the places the engine actually shifts bits?
//! * **2 — chunk index.** The engine is on a dense array. Is the next step (an
//!   arena with `u32` offsets, a hash map) faster, and at which access pattern?
//!
//! Every index benchmark runs **identical loops** through the
//! [`ChunkIndex`] trait, over the **same scattered fixture** the engine would
//! produce by drawing rather than by inserting in key order — see the module
//! docs on [`pixel_engine::perflab::index`] for why that matters.
//!
//! Numbers are `median of N`, milliseconds, and each row prints the ratio to the
//! engine's current choice. A ratio above 1.0 means "slower than what we ship".
use pixel_engine::chunkstore;
use pixel_engine::perflab::index::{ArenaIdx, BTreeIdx, Cell, ChunkIndex, DenseIdx, HashIdx};
use pixel_engine::perflab::maskconst::{
    count_alpha_words, count_alpha_words_bitmap, expand_lanes, mark_range_branchless,
    mark_range_computed, mark_range_table, run_mask_computed, run_mask_table, run_mask_table_2d,
};
use pixel_engine::raster::brush::{stroke_point, stroke_segment};
use std::hint::black_box;
use std::time::Instant;
const RUNS: usize = 9;
// ---------------------------------------------------------------------------
// Harness
fn median_ms(runs: usize, mut f: impl FnMut()) -> f64 {
    let mut s = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        f();
        s.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    s.sort_by(f64::total_cmp);
    s[s.len() / 2]
}
fn row(name: &str, ms: f64, base: Option<f64>) {
    match base {
        Some(b) => println!("{name:<46} | {ms:>9.3} ms | {:>6.2}x", ms / b),
        None => println!("{name:<46} | {ms:>9.3} ms |   base"),
    }
}
fn header(title: &str) {
    println!("\n=== {title} ===");
    println!("{:<46} | {:>12} | {:>7}", "case", "median", "vs engine");
}
// ---------------------------------------------------------------------------
// 1. Mask constants
/// 200k range updates across every legal width — a brush row, a scanline fill
/// and a polygon edge are all just these.
const RANGE_OPS: usize = 200_000;
fn range_lx(i: usize) -> (u32, u32, u32) {
    let ly = (i % 32) as u32;
    let lx = (i / 32 % 32) as u32;
    let len = 1 + (i / 1024 % 32) as u32;
    (ly, lx, lx + len)
}
fn bench_mask_constants() {
    header("1. mask constants");
    let ops: Vec<(u32, u32, u32)> = (0..RANGE_OPS).map(range_lx).collect();
    let base = median_ms(RUNS, || {
        let mut m = chunkstore::EMPTY_MASK;
        for &(ly, x0, x1) in &ops {
            chunkstore::mark_range(&mut m, ly, x0, x1);
        }
        black_box(m);
    });
    row("mark_range — engine (computed)", base, None);
    let t = median_ms(RUNS, || {
        let mut m = chunkstore::EMPTY_MASK;
        for &(ly, x0, x1) in &ops {
            mark_range_computed(&mut m, ly, x0, x1);
        }
        black_box(m);
    });
    row("mark_range — computed copy", t, Some(base));
    let t = median_ms(RUNS, || {
        let mut m = chunkstore::EMPTY_MASK;
        for &(ly, x0, x1) in &ops {
            mark_range_branchless(&mut m, ly, x0, x1);
        }
        black_box(m);
    });
    row("mark_range - branchless, no table", t, Some(base));
    let t = median_ms(RUNS, || {
        let mut m = chunkstore::EMPTY_MASK;
        for &(ly, x0, x1) in &ops {
            mark_range_table(&mut m, ly, x0, x1);
        }
        black_box(m);
    });
    row("mark_range — 64-entry width table", t, Some(base));
    // The pure mask computation, with the caller's loop removed, so the table's
    // win or loss is not hidden by the surrounding address math.
    let pairs: Vec<(u32, u32)> = (0..2_000_000)
        .map(|i| ((i % 64) as u32, 1 + (i / 64 % 64) as u32))
        .filter(|&(s, w)| s + w <= 64)
        .collect();
    let b2 = median_ms(RUNS, || {
        let mut acc = 0u64;
        for &(s, w) in &pairs {
            acc ^= run_mask_computed(s, w);
        }
        black_box(acc);
    });
    row("mask only — computed", b2, None);
    let t = median_ms(RUNS, || {
        let mut acc = 0u64;
        for &(s, w) in &pairs {
            acc ^= run_mask_table(s, w);
        }
        black_box(acc);
    });
    row("mask only — width table", t, Some(b2));
    let t = median_ms(RUNS, || {
        let mut acc = 0u64;
        for &(s, w) in &pairs {
            acc ^= run_mask_table_2d(s, w);
        }
        black_box(acc);
    });
    row("mask only — 64x64 table (32 KB)", t, Some(b2));
    // Would "any alpha here?" be faster without the coverage map?
    let mut chunk = [0u8; 4096];
    for (i, px) in chunk.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        if i % 13 == 0 {
            px[3] = 200;
        }
    }
    let m = chunkstore::from_chunk(&chunk);
    // 400 reps: a single call is ~50 ns, which the optimiser will happily delete.
    const CENSUS_REPS: usize = 400;
    let b3 = median_ms(RUNS, || {
        let mut acc = 0usize;
        for _ in 0..CENSUS_REPS {
            acc += count_alpha_words_bitmap(black_box(&m));
        }
        black_box(acc);
    });
    row("is-there-alpha — coverage bitmap (16 words)", b3, None);
    let t = median_ms(RUNS, || {
        let mut acc = 0usize;
        for _ in 0..CENSUS_REPS {
            acc += count_alpha_words(black_box(&chunk));
        }
        black_box(acc);
    });
    row("is-there-alpha — byte scan (512 u64)", t, Some(b3));
    let words: Vec<u64> = (0..4096).map(|i| if i % 13 == 0 { 1 } else { 0 }).collect();
    let b4 = median_ms(RUNS, || {
        let mut n = 0usize;
        for &w in &words {
            n += expand_lanes(w)[7] as usize;
        }
        black_box(n);
    });
    row("lane expansion — repunit expansion", b4, None);
}
// ---------------------------------------------------------------------------
// 2. Chunk index
/// **Fixture A — a completely full canvas.** Every one of the 2176 chunks exists.
/// This is the opposite extreme from the sparse case and the one where an index
/// has nothing to skip.
fn full_cells() -> Vec<(u32, u32, Cell)> {
    (0..34u32)
        .flat_map(|cy| (0..64u32).map(move |cx| (cx, cy)))
        .map(|(cx, cy)| (cx, cy, Cell::with_span((cx + cy) % 32 + 1)))
        .collect()
}

/// **Fixture B — drawn by the engine's own `stroke_segment`**, so the chunks come
/// into existence in stroke order, not key order. That ordering is the whole
/// point of the arena comparison; see `perflab::index`.
fn busy_cells_via_engine() -> Vec<(u32, u32, Cell)> {
    use pixel_engine::chunkstore::{ChunkStore, PixelWriter};
    let mut store = ChunkStore::new(2048, 1088).expect("valid canvas size");
    {
        let mut w = PixelWriter::new(&mut store, "busy");
        for k in 0..24i32 {
            stroke_segment(
                &mut w,
                (k * 97) % 2048,
                (k * 61) % 1088,
                ((k * 97) + 400) % 2048,
                ((k * 61) + 300) % 1088,
                5,
                0xff00_00ff,
            );
        }
        stroke_point(&mut w, 1024, 544, 5, 0xff00_00ff);
        w.finish();
    }
    let mut out = Vec::new();
    store.for_each_chunk(|cx, cy, _| out.push((cx, cy, Cell::with_span(17))));
    out
}

fn bench_index(cells: &[(u32, u32, Cell)]) {
    header("2. chunk index");
    println!("fixture: {} chunks", cells.len());

    let cx_total = 64u32;
    let cy_total = 34u32;

    // --- access pattern 1: the composite sweep. Chunk-row major, key ascending,
    //     every layer resolving every chunk. This is the docs' §4-6 workload.
    macro_rules! sweep {
        ($idx:expr) => {{
            let idx = $idx;
            median_ms(RUNS, || {
                let mut acc = 0u64;
                for cy in 0..cy_total {
                    for cx in 0..cx_total {
                        if let Some(m) = idx.mask(cx, cy) {
                            for (w, &word) in m.iter().enumerate() {
                                acc ^= word.rotate_left(((cx + cy) as usize + w) as u32 & 63);
                            }
                        }
                    }
                }
                black_box(acc);
            })
        }};
    }

    let base = sweep!(BTreeIdx::build(cx_total, cy_total, cells));
    row("composite sweep  - BTreeMap", base, None);
    let t = sweep!(DenseIdx::build(cx_total, cy_total, cells));
    row("composite sweep  - dense array (ENGINE)", t, Some(base));
    let t = sweep!(ArenaIdx::build(cx_total, cy_total, cells));
    row("composite sweep  - arena (drawn order)", t, Some(base));
    let t = sweep!(ArenaIdx::build_in_key_order(cx_total, cy_total, cells));
    row("composite sweep  - arena (KEY order*)", t, Some(base));
    let t = sweep!(HashIdx::build(cx_total, cy_total, cells));
    row("composite sweep  - HashMap", t, Some(base));

    // --- access pattern 2: flood fill's row scan. One chunk per 32 pixels, so a
    //     bounded stride over mostly-absent chunks.
    macro_rules! rowscan {
        ($idx:expr) => {{
            let idx = $idx;
            median_ms(RUNS, || {
                let mut acc = 0u64;
                for cy in 0..cy_total {
                    for cx in 0..cx_total {
                        acc ^= idx
                            .chunk(cx, cy)
                            .map_or(0, |c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64);
                    }
                }
                black_box(acc);
            })
        }};
    }
    let base = rowscan!(BTreeIdx::build(cx_total, cy_total, cells));
    row("row scan (fill)  - BTreeMap", base, None);
    let t = rowscan!(DenseIdx::build(cx_total, cy_total, cells));
    row("row scan (fill)  - dense array (ENGINE)", t, Some(base));
    let t = rowscan!(ArenaIdx::build(cx_total, cy_total, cells));
    row("row scan (fill)  - arena (drawn order)", t, Some(base));
    let t = rowscan!(HashIdx::build(cx_total, cy_total, cells));
    row("row scan (fill)  - HashMap", t, Some(base));

    // --- access pattern 3: a single pen-64 write. The engine's worst probe
    //     rate: ~5.2M chunk resolutions for one bench item.
    macro_rules! pen64 {
        ($idx:expr) => {{
            let mut idx = $idx;
            median_ms(5, || {
                for seg in 0..20i32 {
                    let y = (seg * 29) % 1088;
                    let mut x = 10i32;
                    while x < 2038 {
                        for by in 0..64i32 {
                            let yy = y + by - 32;
                            if (0..1088).contains(&yy) {
                                let d = idx.chunk_mut((x as u32) >> 5, (yy as u32) >> 5);
                                let o = (((yy as u32) & 31) * 32 + ((x as u32) & 31)) as usize * 4;
                                d[o..o + 4].copy_from_slice(&[1, 2, 3, 255]);
                            }
                        }
                        x += 1;
                    }
                }
                black_box(&idx);
            })
        }};
    }
    let base = pen64!(BTreeIdx::build(cx_total, cy_total, cells));
    row("pen-64 write      - BTreeMap", base, None);
    let t = pen64!(DenseIdx::build(cx_total, cy_total, cells));
    row("pen-64 write      - dense array (ENGINE)", t, Some(base));
    let t = pen64!(ArenaIdx::build(cx_total, cy_total, cells));
    row("pen-64 write      - arena (drawn order)", t, Some(base));
    let t = pen64!(HashIdx::build(cx_total, cy_total, cells));
    row("pen-64 write      - HashMap", t, Some(base));

    // --- access pattern 4: the undo snapshot. Nobody benchmarks this, but a
    //     stroke records one per edit.
    macro_rules! snap {
        ($idx:expr) => {{
            let idx = $idx;
            median_ms(RUNS, || {
                let mut acc = 0usize;
                for _ in 0..20 {
                    let s = idx.snapshot();
                    acc += s.len();
                    black_box(&s);
                }
                black_box(acc);
            })
        }};
    }
    let base = snap!(BTreeIdx::build(cx_total, cy_total, cells));
    row("snapshot x20      - BTreeMap", base, None);
    let t = snap!(DenseIdx::build(cx_total, cy_total, cells));
    row("snapshot x20      - dense array (ENGINE)", t, Some(base));
    let t = snap!(ArenaIdx::build(cx_total, cy_total, cells));
    row("snapshot x20      - arena (drawn order)", t, Some(base));
    let t = snap!(HashIdx::build(cx_total, cy_total, cells));
    row("snapshot x20      - HashMap", t, Some(base));

    // --- and the one the serializer pays: key-ascending walk, no sort allowed.
    macro_rules! walk {
        ($idx:expr) => {{
            let idx = $idx;
            median_ms(RUNS, || {
                let mut acc = 0u64;
                idx.for_each(&mut |_, _, c| acc = acc.wrapping_add(c.mask[0]));
                black_box(acc);
            })
        }};
    }
    let base = walk!(BTreeIdx::build(cx_total, cy_total, cells));
    row("ordered walk      - BTreeMap", base, None);
    let t = walk!(DenseIdx::build(cx_total, cy_total, cells));
    row("ordered walk      - dense array (ENGINE)", t, Some(base));
    let t = walk!(ArenaIdx::build(cx_total, cy_total, cells));
    row("ordered walk      - arena (drawn order)", t, Some(base));
    let t = walk!(HashIdx::build(cx_total, cy_total, cells));
    row("ordered walk      - HashMap (+sort)", t, Some(base));
}

fn main() {
    println!("[pixeldraw] perflab - candidate vs engine");
    println!("2048x1088 chunk grid, rustc release profile\n");

    bench_mask_constants();

    let cells = full_cells();
    println!("\n[fixture A] every chunk of the grid populated");
    bench_index(&cells);
    let cells = busy_cells_via_engine();
    println!("\n[fixture B] drawn by the real stroke_segment");
    bench_index(&cells);

    println!("\n* 'KEY order' allocates chunks in ascending chunk_key - what the");
    println!("  engine does NOT do. It is the ceiling of the arena idea, not a candidate.");
}
