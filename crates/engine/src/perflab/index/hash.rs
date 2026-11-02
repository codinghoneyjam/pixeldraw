use super::*;

// HashMap — closest to the JS side's `Map`.

pub struct HashIdx {
    map: HashMap<u64, Cell>,
    cx_total: u32,
    cy_total: u32,
}
impl HashIdx {
    #[inline]
    fn in_range(&self, cx: u32, cy: u32) -> bool {
        cx < self.cx_total && cy < self.cy_total
    }
}

impl ChunkIndex for HashIdx {
    fn build(cx_total: u32, cy_total: u32, cells: &[(u32, u32, Cell)]) -> Self {
        HashIdx {
            map: cells
                .iter()
                .map(|&(cx, cy, ref c)| (chunk_key(cx, cy), c.clone()))
                .collect(),
            cx_total,
            cy_total,
        }
    }
    fn chunk(&self, cx: u32, cy: u32) -> Option<&[u8; CHUNK_LEN]> {
        if !self.in_range(cx, cy) {
            return None;
        }
        self.map.get(&chunk_key(cx, cy)).map(|c| &*c.data)
    }
    fn mask(&self, cx: u32, cy: u32) -> Option<&AlphaMask> {
        if !self.in_range(cx, cy) {
            return None;
        }
        self.map.get(&chunk_key(cx, cy)).map(|c| &c.mask)
    }
    fn chunk_mut(&mut self, cx: u32, cy: u32) -> &mut [u8; CHUNK_LEN] {
        &mut self
            .map
            .entry(chunk_key(cx, cy))
            .or_insert_with(Cell::blank)
            .data
    }
    fn remove(&mut self, cx: u32, cy: u32) {
        self.map.remove(&chunk_key(cx, cy));
    }
    /// **Has to sort**, and that is the point: `HashMap` iteration order is
    /// unspecified, so contract §2 rule 8 (deterministic serialization) is not
    /// free with this index — it costs a full sort on every save.
    fn for_each(&self, f: &mut dyn FnMut(u32, u32, &Cell)) {
        let mut keys: Vec<u64> = self.map.keys().copied().collect();
        keys.sort_unstable();
        for k in keys {
            f(
                (k % MAX_CHUNKS_X as u64) as u32,
                (k / MAX_CHUNKS_X as u64) as u32,
                &self.map[&k],
            );
        }
    }
    fn len(&self) -> usize {
        self.map.len()
    }
    fn snapshot(&self) -> Self {
        HashIdx {
            map: self.map.clone(),
            cx_total: self.cx_total,
            cy_total: self.cy_total,
        }
    }
}
