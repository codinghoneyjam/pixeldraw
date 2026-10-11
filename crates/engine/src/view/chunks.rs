//! `visibleChunkRange` — which chunks a viewport can see.

use crate::view::types::View;

/// Re-exported so `crate::view::chunks::…` keeps resolving; the canonical home
/// is [`crate::view::types`].
pub use crate::view::types::VisibleChunkRange;

pub const CHUNK_EDGE_EPS: f64 = 1e-9;

/// The far-edge `- 1e-9` is the single most important detail here.
///
/// It keeps a canvas edge landing exactly on a chunk boundary from over-reporting
/// by one chunk: with `canvas = 128`, viewport `64`, zoom 1, the far edge sits on
/// canvas x=64 and `floor(64/32) == 2` would claim chunk 2, which covers canvas
/// 64..95 and is off screen. Subtracting the epsilon floors to 1. Without it the
/// renderer composites a chunk nobody can see.
///
/// The near edge clamps at zero instead: chunks left of or above the canvas do
/// not exist. `None` is the JS's "scrolled fully off screen".
pub fn visible_chunk_range(
    v: View,
    canvas_w: f64,
    canvas_h: f64,
    vw: f64,
    vh: f64,
) -> Option<VisibleChunkRange> {
    let a = crate::view::geom::screen_to_canvas(v, 0.0, 0.0);
    let b = crate::view::geom::screen_to_canvas(v, vw, vh);
    let chunk = crate::constants::CHUNK_PX as f64;

    let cx0 = (a.x / chunk).floor().max(0.0);
    let cy0 = (a.y / chunk).floor().max(0.0);
    // Far edges are capped by the canvas, so the near edges can never exceed
    // them; the comparison below turns an inverted range into `None`.
    let cx1 = (canvas_w / chunk - 1.0).min(((b.x - CHUNK_EDGE_EPS) / chunk).floor());
    let cy1 = (canvas_h / chunk - 1.0).min(((b.y - CHUNK_EDGE_EPS) / chunk).floor());

    if cx1 < cx0 || cy1 < cy0 {
        return None;
    }
    Some(VisibleChunkRange {
        cx0: floor_u32(cx0),
        cy0: floor_u32(cy0),
        cx1: floor_u32(cx1),
        cy1: floor_u32(cy1),
    })
}

/// `floor()` first for the same reason as [`crate::view::geom::floor_i64`], and
/// `.max(0.0)` before the cast so the value reaching `as u32` is representable.
#[inline]
fn floor_u32(v: f64) -> u32 {
    v.floor().max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::types::View;

    #[test]
    fn epsilon_stops_the_far_edge_over_reporting() {
        // Canvas 128, viewport 64, zoom 1: the far edge is exactly canvas x=64.
        let v = View::new(1.0, 0.0, 0.0).expect("valid");
        let r = visible_chunk_range(v, 128.0, 128.0, 64.0, 64.0).expect("some");
        assert_eq!((r.cx0, r.cx1), (0, 1));
        assert_eq!((r.cy0, r.cy1), (0, 1));
    }

    #[test]
    fn near_edge_clamps_but_the_far_one_does_not() {
        let v = View::new(1.0, 20.0, 20.0).expect("valid");
        let r = visible_chunk_range(v, 128.0, 128.0, 64.0, 64.0).expect("some");
        assert_eq!(r.cx0, 0, "the near edge clamps at zero");
        assert!(r.cx1 <= 3, "the far edge stays inside the canvas");
        // 128/32 = 4 chunks, so the largest legal far index is 3.
        assert!(r.cx1 < 128 / crate::constants::CHUNK_PX);
    }

    #[test]
    fn fully_off_screen_is_none() {
        let v = View::new(1.0, 500.0, 500.0).expect("valid");
        assert!(visible_chunk_range(v, 128.0, 128.0, 64.0, 64.0).is_none());
    }
}
