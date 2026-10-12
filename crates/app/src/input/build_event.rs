//! `src/tools/pointer_event.js` — host pointer payload → [`ToolEvent`].
//!
//! Pure mapping: no listener wiring, no egui/winit types. The host rect has
//! already been subtracted by whoever fills [`RawPointer::pos`], which is the
//! stand-in for `clientX - hostRect.left` in the JS original.

use crate::input::event::{Button, PointerKind, ToolEvent};
use pixel_engine::view::{View, floor_i64, pixel_at, screen_to_canvas};

/// A host pointer event reduced to what `buildToolEvent` consumes.
///
/// `pos` is **host-relative CSS (logical) px**. `coalesced` carries the
/// intermediate positions a frame coalesced into one event, oldest first; the
/// browser's own list already ends at the current position, exactly like the
/// DOM `getCoalescedEvents()` result the JS read.
#[derive(Clone, Debug, PartialEq)]
pub struct RawPointer {
    pub pos: (f64, f64),
    pub coalesced: Vec<(f64, f64)>,
    pub button: Button,
    pub buttons_down: u8,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub pointer_id: u64,
    pub kind: PointerKind,
    pub time_ms: f64,
}

impl Default for RawPointer {
    fn default() -> Self {
        Self {
            pos: (0.0, 0.0),
            coalesced: Vec::new(),
            button: Button::Left,
            buttons_down: 0,
            shift: false,
            ctrl: false,
            alt: false,
            // DOM pointerId 0 is a real id, so keep it out of the neutral value.
            pointer_id: 1,
            kind: PointerKind::Mouse,
            time_ms: 0.0,
        }
    }
}

impl RawPointer {
    pub fn new(pos: (f64, f64)) -> Self {
        Self {
            pos,
            ..Self::default()
        }
    }
}

/// `buildToolEvent(domEv, view, hostRect)` with `hostRect == {left: 0, top: 0}`.
pub fn build_tool_event(raw: &RawPointer, view: &View) -> ToolEvent {
    let (sx, sy) = raw.pos;
    let f = screen_to_canvas(*view, sx, sy);
    let p = pixel_at(*view, sx, sy);
    let mut pts: Vec<(i32, i32)> = Vec::new();
    if raw.coalesced.is_empty() {
        // empty getCoalescedEvents() and a throwing one both fall back to [domEv]
        push_pixel(&mut pts, view, sx, sy);
    } else {
        for &(cx, cy) in &raw.coalesced {
            push_pixel(&mut pts, view, coord(cx, sx), coord(cy, sy));
        }
    }
    // Trap: the last element is always the primary pixel, so a consumer can walk
    // `coalesced` without ever reading past the current position.
    let here = (to_i32(p.x), to_i32(p.y));
    match pts.last() {
        None => pts.push(here),
        Some(&last) if last != here => pts.push(here),
        Some(_) => {}
    }
    ToolEvent {
        x: here.0,
        y: here.1,
        fx: f.x,
        fy: f.y,
        sx,
        sy,
        button: raw.button,
        shift: raw.shift,
        // JS `ctrlKey === true || metaKey === true` is already collapsed by the host.
        ctrl: raw.ctrl,
        alt: raw.alt,
        pointer_id: raw.pointer_id,
        pointer_kind: raw.kind,
        coalesced: pts,
        time_ms: raw.time_ms,
    }
}

/// `Math.floor(screenToCanvas(...))`, with consecutive duplicates skipped.
fn push_pixel(pts: &mut Vec<(i32, i32)>, view: &View, sx: f64, sy: f64) {
    let c = screen_to_canvas(*view, sx, sy);
    let px = (to_i32(floor_i64(c.x)), to_i32(floor_i64(c.y)));
    if pts.last().copied() == Some(px) {
        return;
    }
    pts.push(px);
}

/// JS `typeof e.clientX === "number" ? e.clientX - rect.left : sx`: a coalesced
/// entry without usable coordinates borrows the primary point's.
fn coord(v: f64, fallback: f64) -> f64 {
    if v.is_finite() { v } else { fallback }
}

/// `Math.floor` yields an i64; the frozen `ToolEvent` contract stores i32.
fn to_i32(v: i64) -> i32 {
    v as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View::new(1.0, 0.0, 0.0).expect("valid view")
    }

    fn zoomed() -> View {
        View::new(4.0, 0.0, 0.0).expect("valid view")
    }

    /// Trap: the last `coalesced` element is always `(x, y)`, whatever the list.
    #[test]
    fn coalesced_ends_at_the_primary_pixel() {
        let raw = RawPointer {
            pos: (11.0, 11.0),
            coalesced: vec![(2.0, 2.0), (3.5, 3.5), (3.6, 3.6), (10.0, 10.0)],
            ..RawPointer::default()
        };
        let ev = build_tool_event(&raw, &view());
        assert_eq!(ev.coalesced, vec![(2, 2), (3, 3), (10, 10), (11, 11)]);
        assert_eq!(ev.coalesced.last().copied(), Some((ev.x, ev.y)));
        assert_eq!((ev.x, ev.y), (11, 11));
    }

    #[test]
    fn an_empty_coalesced_list_falls_back_to_the_primary_point() {
        let raw = RawPointer::new((11.0, 11.0));
        let ev = build_tool_event(&raw, &view());
        assert_eq!(ev.coalesced, vec![(11, 11)]);
    }

    /// A coalesced point that lands on the same pixel as its predecessor is
    /// dropped, so a consumer never strokes a zero-length segment.
    #[test]
    fn consecutive_duplicates_are_skipped() {
        let raw = RawPointer {
            pos: (3.9, 3.9),
            coalesced: vec![(0.0, 0.0), (0.9, 0.9), (1.9, 1.9), (2.9, 2.9)],
            ..RawPointer::default()
        };
        let ev = build_tool_event(&raw, &view());
        assert_eq!(ev.coalesced, vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
    }

    /// JS `Math.floor`, not truncation: a pointer above/left of the canvas
    /// legitimately reads `-1`.
    #[test]
    fn negative_screen_coords_floor_down() {
        let raw = RawPointer {
            pos: (-0.5, -0.5),
            coalesced: vec![(-0.5, -0.5)],
            ..RawPointer::default()
        };
        let ev = build_tool_event(&raw, &view());
        assert_eq!((ev.x, ev.y), (-1, -1));
        assert_eq!(ev.coalesced, vec![(-1, -1)]);
    }

    #[test]
    fn a_coalesced_point_without_coordinates_borrows_the_primary() {
        let raw = RawPointer {
            pos: (7.25, 8.75),
            coalesced: vec![(f64::NAN, f64::NAN)],
            ..RawPointer::default()
        };
        let ev = build_tool_event(&raw, &view());
        assert_eq!(ev.coalesced, vec![(7, 8)]);
        assert_eq!(ev.coalesced.last().copied(), Some((ev.x, ev.y)));
    }

    /// The rapid multi-point sequence: many points, one pixel each, and the
    /// invariant still holds at the end.
    #[test]
    fn rapid_multi_point_sequence_keeps_the_invariant() {
        let mut coalesced = Vec::new();
        for i in 0..64 {
            coalesced.push((i as f64 * 3.0, (63 - i) as f64 * 3.0));
        }
        let raw = RawPointer {
            pos: (500.0, 0.0),
            coalesced,
            ..RawPointer::default()
        };
        let ev = build_tool_event(&raw, &view());
        assert_eq!(ev.coalesced.last().copied(), Some((ev.x, ev.y)));
        for pair in ev.coalesced.windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
    }

    #[test]
    fn zoom_and_offset_are_applied_to_every_point() {
        let raw = RawPointer::new((9.0, 13.0));
        let ev = build_tool_event(&raw, &zoomed());
        assert_eq!((ev.x, ev.y), (2, 3));
        assert_eq!((ev.fx, ev.fy), (2.25, 3.25));
        assert_eq!((ev.sx, ev.sy), (9.0, 13.0));
    }

    #[test]
    fn modifiers_and_button_are_copied_through() {
        let raw = RawPointer {
            button: Button::Middle,
            buttons_down: 2,
            shift: true,
            ctrl: true,
            alt: true,
            pointer_id: 7,
            kind: PointerKind::Pen,
            time_ms: 12.5,
            ..RawPointer::new((1.0, 1.0))
        };
        let ev = build_tool_event(&raw, &view());
        assert_eq!(ev.button, Button::Middle);
        assert!(ev.shift && ev.ctrl && ev.alt);
        assert_eq!(ev.pointer_id, 7);
        assert_eq!(ev.pointer_kind, PointerKind::Pen);
        assert_eq!(ev.time_ms, 12.5);
    }
}
