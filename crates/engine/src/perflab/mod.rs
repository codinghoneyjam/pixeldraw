//! **Module 2 — the performance lab. Nothing here is wired into the engine.**
//!
//! This is a measurement harness, not an implementation. Every production path
//! still goes through [`crate::chunkstore`] / [`crate::composite`] exactly as it
//! did; nothing in this module is called from there, and deleting the whole
//! directory would change no byte of engine behaviour. It exists so that a
//! candidate can be *measured* before anyone decides to adopt it.
//!
//! Two candidates are under test, both raised as "there must be more available":
//!
//! 1. **Precomputed mask constants** (`maskconst`) — the bit-trick habit of
//!    writing `1`, `11`, `111`, `1111`, … once instead of shifting per call.
//!    Applied where the engine actually shifts (`alpha::mark_range`) and applied
//!    where it might plausibly help instead (expanding a coverage word into
//!    per-lane bytes, and SWAR zero-detection over the alpha plane).
//! 2. **A different chunk index** (`index`) — the engine went
//!    `BTreeMap → dense array`; this lab asks what comes *after* the dense array:
//!    an arena with `u32` offsets, a hash map, and each one's access pattern.
//!
//! Read [`crate::examples::perflab`] for the numbers. The rule this module
//! follows: **a candidate that does not win does not get adopted**, and the
//! write-up records the losers as prominently as the winners.

pub mod index;
pub mod maskconst;
