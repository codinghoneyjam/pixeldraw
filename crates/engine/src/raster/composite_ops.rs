//! Read-back composite operations, ported from `src/core/raster/composite_ops.js`
//! — facade.
//!
//! None of these can be a mask-and-fill primitive: every one reads what is
//! already on the tile through `PixelSink::get`, because the legacy bakers
//! composed their slots that way.
//!
//! * [`clip_alpha`] — multiply a body slot's alpha by a clip mask
//! * [`erase_polygon`] — punch `cutouts` polygons to alpha 0
//! * [`visor_layer`] — fill a shape, then composite CRT scanlines masked by it
//! * [`sheen_overlay`] — two diagonal white lines masked to the visor shape
//! * [`hud_brackets`] — eight L-shaped corner segments
//!
//! The sheen's three-stage order is load-bearing; see `docs/raster_internals.md`
//! §5 for why compositing first and masking after yields a different image.

mod clip;
mod common;
mod erase;
mod hud;
mod sheen;
mod visor;

#[cfg(test)]
mod tests;

pub use clip::{Clip, clip_alpha};
pub use erase::erase_polygon;
pub use hud::{Hud, hud_brackets};
pub use sheen::{Sheen, sheen_overlay};
pub use visor::{CrtLines, RecipeShape, VisorShape, shape_fill_mask, visor_layer};
