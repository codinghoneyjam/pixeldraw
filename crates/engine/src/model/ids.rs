//! Counter-based ids — the `IdGen` of `src/features/document/document.js`.
//!
//! Layer ids are **never random**; the fixtures pin `layer-0001`, so every id
//! comes from a counter that is seeded from whatever ids a loaded document
//! already holds. The document id (`doc-0001`) comes from a module-global
//! counter, mirroring the JS module-level `docCounter`.

use std::sync::atomic::{AtomicU64, Ordering};

/// `/^layer-(\d+)$/` — the only shape [`IdGen::seed`] recognises.
const LAYER_ID_DIGITS: &str = "layer-";

/// The document counter behind `doc-0001`; module-global, monotonic.
static DOC_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Counter-based id generator.
///
/// The counter is seeded from whatever ids a loaded document already holds, so a
/// re-saved file keeps issuing fresh ids instead of colliding with the ones it
/// brought in.
#[derive(Debug, Clone, Default)]
pub struct IdGen {
    counter: u64,
}

impl IdGen {
    pub fn new() -> Self {
        Self::default()
    }

    /// `seed(ids)` — take the max numeric suffix, ignoring everything else.
    ///
    /// `"background"` and `"shape-9"` do not match the `layer-\d+` shape and are
    /// skipped, so they cannot push the counter anywhere. The JS `/^layer-(\d+)$/`
    /// also rejects `layer-` with no digits, `layer-abc` and any upper-case
    /// prefix, all of which fall out of the strip + digit check below.
    pub fn seed(&mut self, ids: impl IntoIterator<Item = impl AsRef<str>>) {
        for id in ids {
            let id = id.as_ref();
            let Some(digits) = id.strip_prefix(LAYER_ID_DIGITS) else {
                continue;
            };
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if let Ok(n) = digits.parse::<u64>()
                && n > self.counter
            {
                self.counter = n;
            }
        }
    }

    /// `next()` — pre-increment, then `padStart(4, "0")`.
    ///
    /// `String(10000).padStart(4, "0")` is `"10000"`, not a truncation, so a
    /// document that has already consumed 9999 ids keeps growing.
    ///
    /// The name is the JS one and is pinned by the tests, so it is kept even
    /// though it collides in spirit with `Iterator::next` — implementing that
    /// trait would make the counter an iterator, which it is not.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> String {
        self.counter += 1;
        format!("{LAYER_ID_DIGITS}{:04}", self.counter)
    }

    pub fn counter(&self) -> u64 {
        self.counter
    }
}

/// The next `doc-NNNN`.
pub fn next_doc_id() -> String {
    let n = DOC_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    format!("doc-{n:04}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // <META - ROLE : parity with tests/model_basic.test.mjs B5.2 IdGen cases | L48-58>
    #[test]
    fn idgen_seeds_from_known_ids_and_ignores_the_rest() {
        let mut ids_gen = IdGen::new();
        ids_gen.seed(["layer-0007", "background", "layer-0003", "shape-9"]);
        assert_eq!(ids_gen.next(), "layer-0008");
        assert_eq!(ids_gen.next(), "layer-0009");
    }

    #[test]
    fn idgen_pads_to_four_then_grows() {
        let mut ids_gen = IdGen::new();
        ids_gen.seed(["layer-9999"]);
        assert_eq!(ids_gen.next(), "layer-10000");
    }

    #[test]
    fn idgen_ignores_malformed_ids() {
        let mut ids_gen = IdGen::new();
        ids_gen.seed([
            "layer-",
            "layer-abc",
            "layer-0x5",
            "LAYER-0009",
            "layer-0004",
        ]);
        assert_eq!(ids_gen.counter(), 4);
        assert_eq!(ids_gen.next(), "layer-0005");
    }

    #[test]
    fn doc_ids_are_padded_and_unique() {
        assert!(next_doc_id().starts_with("doc-"));
        assert_ne!(next_doc_id(), next_doc_id());
    }
}
