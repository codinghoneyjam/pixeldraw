//! File format codecs — the Rust side of `src/io/`.
//!
//! * [`base64`] — the chunk-payload text encoding (JS `io/base64.js`).
//! * [`png`] — the 8-bit RGBA PNG codec (JS `io/png.js`).
//! * [`export`] — `flattenToRgba`, the file path of the compositor
//!   (JS `io/export_png.js`).
//!
//! The seven browser-storage modules (`file_io.js`, `idb_*.js`, `store_idb.js`)
//! have **no counterpart here**: dialogs, the app directory and autosave belong
//! to `platform`, not to a pure-engine codec (docs/native_plan.md §5).
//!
//! Both codecs raise [`DrawToolError`] codes from the frozen catalog rather than
//! their own error types, so a caller sees the same failure vocabulary whatever
//! went wrong.

pub mod base64;
pub mod export;
pub mod png;

pub use base64::{base64_to_bytes, bytes_to_base64};
pub use export::{FlattenOptions, flatten_to_rgba};
pub use png::{PngImage, decode_png, encode_png};
