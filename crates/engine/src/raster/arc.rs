//! PIL-style arc / chord masks, ported from `src/core/raster/arc.js` — facade.
//!
//! `ImageDraw.arc` and `ImageDraw.chord` stroke a *sampled ellipse perimeter*,
//! so an arc is not an analytic curve — it is a polyline of ~0.5° samples, each
//! segment going through the same `line8` / wide-quad path as everything else.
//! That is why this module is thin: `arc_ellipse_points` plus the primitives
//! from `super::polygon`.
//!
//! **Near-parity is deliberate and pinned in [`near_parity`].** A port that
//! matched PIL *better* would fail the golden gate; the bounds are asserted by
//! the tests here. Rationale: `docs/raster_internals.md` §3.

mod angle;
mod chord;

#[cfg(test)]
mod tests;

pub use angle::{Box2, arc_ellipse_points, arc_mask, near_parity};
pub use chord::{chord_fill_mask, chord_outline_mask};
