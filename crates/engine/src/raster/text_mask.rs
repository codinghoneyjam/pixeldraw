//! Bitmap-glyph text raster, ported from `src/core/raster/text_mask.js` — facade.
//!
//! `recipe/text.rs` keeps its own private glyph reader (that module is outside
//! this task's scope), so this tree owns a second copy of the glyph types.

mod glyph;
mod place;
mod stamp;

pub use glyph::{Glyph, GlyphSet, GlyphShape, decode_glyph_sets};
pub use place::{PlacedGlyph, place_text};
pub use stamp::{TextParams, text_mask};
