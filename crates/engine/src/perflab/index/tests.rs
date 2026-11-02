use super::*;

fn fixture(n: usize) -> Vec<(u32, u32, Cell)> {
    (0..n)
        .map(|i| {
            (
                i as u32 % 40,
                (i / 40) as u32,
                Cell::with_span((i % 32) as u32),
            )
        })
        .collect()
}

fn check<I: ChunkIndex>(idx: &I, cells: &[(u32, u32, Cell)]) {
    assert_eq!(idx.len(), cells.len(), "count");
    for &(cx, cy, _) in cells {
        assert!(idx.chunk(cx, cy).is_some(), "missing ({cx},{cy})");
        assert!(idx.mask(cx, cy).is_some(), "missing mask ({cx},{cy})");
    }
    assert!(
        idx.chunk(64, 0).is_none(),
        "cx past the canvas must be absent"
    );
    assert!(
        idx.chunk(0, 34).is_none(),
        "cy past the canvas must be absent"
    );
    // Key order, which the serializer depends on.
    let mut seen = Vec::new();
    idx.for_each(&mut |cx, cy, _| seen.push(chunk_key(cx, cy)));
    let mut sorted = seen.clone();
    sorted.sort_unstable();
    assert_eq!(seen, sorted, "iteration must be key-ascending");
}

#[test]
fn every_index_holds_the_same_map() {
    let cells = fixture(300);
    check(&BTreeIdx::build(64, 34, &cells), &cells);
    check(&DenseIdx::build(64, 34, &cells), &cells);
    check(&ArenaIdx::build(64, 34, &cells), &cells);
    check(&ArenaIdx::build_in_key_order(64, 34, &cells), &cells);
    check(&HashIdx::build(64, 34, &cells), &cells);
}

#[test]
fn remove_and_reinsert_reuses_without_growing_the_arena() {
    let cells = fixture(100);
    let mut idx = ArenaIdx::build(64, 34, &cells);
    let before = idx.arena.len();
    for &(cx, cy, _) in &cells {
        idx.remove(cx, cy);
    }
    assert_eq!(idx.len(), 0);
    for &(cx, cy, _) in &cells {
        idx.chunk_mut(cx, cy)[0] = 9;
    }
    assert_eq!(idx.len(), cells.len());
    assert_eq!(idx.arena.len(), before, "free list must be reused in place");
    assert_eq!(idx.chunk(cells[0].0, cells[0].1).expect("present")[0], 9);
}

#[test]
fn snapshots_are_independent() {
    let cells = fixture(50);
    let mut idx = DenseIdx::build(64, 34, &cells);
    let snap = idx.snapshot();
    *idx.chunk_mut(0, 0) = [7; CHUNK_LEN];
    assert_ne!(snap.chunk(0, 0).expect("present")[0], 7);
}

#[test]
fn the_arena_and_the_dense_index_return_identical_bytes() {
    let cells = fixture(200);
    let dense = DenseIdx::build(64, 34, &cells);
    let arena = ArenaIdx::build(64, 34, &cells);
    for &(cx, cy, ref c) in &cells {
        assert_eq!(dense.chunk(cx, cy), Some(&*c.data), "dense ({cx},{cy})");
        assert_eq!(arena.chunk(cx, cy), Some(&*c.data), "arena ({cx},{cy})");
        assert_eq!(dense.mask(cx, cy), Some(&c.mask));
        assert_eq!(arena.mask(cx, cy), Some(&c.mask));
    }
}
