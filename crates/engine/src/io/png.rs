//! PNG codec, ported from `src/io/png.js` — facade.
//!
//! Scope is deliberately the same as the JS one: **8-bit RGBA, non-interlaced**
//! on read; filter-0 rows and a single IDAT on write. Nothing else is accepted,
//! and the failure is a coded [`DrawToolError`], never a panic.
//!
//! `encodePng`/`decodePng` were `async` only because they went through
//! `CompressionStream`; with `flate2` they are plain functions. The compressed
//! bytes are **not** byte-identical to the JS output — the plan accepts this
//! (난제 4): the gate is *decoded pixel* equality, plus a save→reload round
//! trip. Error-code boundaries: `docs/raster_internals.md` §7.

mod crc;
mod decode;
mod encode;

#[cfg(test)]
mod tests;

pub use decode::{PngImage, decode_png};
pub use encode::encode_png;
