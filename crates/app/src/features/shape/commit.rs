//! `features/shape/shape_render.js` commit half — colour, spec, history write.
//!
//! The pending colour is a draw-time snapshot; size/fill/radius stay live.
//! Commit opens one stroke on the pending's layer and closes it, so undo is a
//! single step.

use pixel_engine::chunkstore::PixelWriter;
use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::model::edit_session::EditOptions;
use pixel_engine::pixel::{pack_rgba, parse_hex};
use pixel_engine::raster::shape_raster::{ShapeKind, ShapeSpec, apply_shape};

use super::geom::{PendingGeom, build_spec};
use super::state::PendingShape;
use crate::tools::api::ToolCx;

/// `buildColor` — the pending snapshot wins; otherwise the live setting.
pub(super) fn build_color(pending: Option<&PendingShape>, primary_color: &str) -> String {
    pending
        .map(|p| p.color.clone())
        .unwrap_or_else(|| primary_color.to_string())
}

/// `packOf` — normalized css hex to packed, alpha preserved.
pub(super) fn pack_of(css: &str) -> EngineResult<u32> {
    match parse_hex(css) {
        Some(c) => Ok(pack_rgba(
            c[0] as f64,
            c[1] as f64,
            c[2] as f64,
            c[3] as f64,
        )),
        None => Err(DrawToolError::out_of_range(format!("invalid color {css}"))),
    }
}

fn commit_error_code(code: &str) -> bool {
    // `commitPending` discards on locked/hidden/missing layers, rethrows rest.
    matches!(code, "LAYER_LOCKED" | "LAYER_HIDDEN" | "LAYER_NOT_FOUND")
}

/// What `commitPending` did with the pending shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CommitResult {
    Committed,
    /// Layer locked/hidden/gone — the JS discarded the pending shape.
    Discarded,
    /// No pending shape; nothing happened.
    Empty,
}

/// `commitPending` — apply the pending shape as one history entry.
pub(super) fn commit_pending(
    kind: ShapeKind,
    label: &str,
    pending: Option<PendingShape>,
    cx: &mut ToolCx,
) -> EngineResult<CommitResult> {
    let Some(shape) = pending else {
        return Ok(CommitResult::Empty);
    };
    if let Err(e) = cx
        .session
        .stroke_begin(EditOptions::on_layer(&shape.layer_id).labeled(label))
    {
        if commit_error_code(e.code) {
            return Ok(CommitResult::Discarded);
        }
        return Err(e);
    }
    let s = cx.session.settings();
    let color = build_color(Some(&shape), &s.primary_color);
    let pack = pack_of(&color)?;
    let spec = build_spec(kind, &shape.geom, s.pen_size, s.shape_fill);
    // Foreground only — the background slot is a buffer, passed as the same
    // colour so it can never paint.
    let result = cx
        .session
        .stroke_with(|w| apply_shape(w, &spec, pack, pack));
    match result {
        Ok(Ok(_)) => {}
        Ok(Err(e)) | Err(e) => {
            // The JS leaked the edit here; cancelling restores the pixels and
            // keeps the session usable instead.
            cx.session.stroke_cancel();
            return Err(e);
        }
    }
    cx.session.stroke_commit(Some(label))?;
    Ok(CommitResult::Committed)
}

/// `paintPreview` — WYSIWYG pixels into a scratch writer. Returns the count.
pub fn paint_preview(
    kind: ShapeKind,
    geom: &PendingGeom,
    color: &str,
    pen_size: u32,
    shape_fill: &str,
    writer: &mut PixelWriter<'_>,
) -> EngineResult<usize> {
    let pack = pack_of(color)?;
    let spec = build_spec(kind, geom, pen_size, shape_fill);
    apply_shape(writer, &spec, pack, pack)
}

/// Commit-time [`ShapeSpec`] for tests and the overlay.
pub fn commit_spec(
    kind: ShapeKind,
    geom: &PendingGeom,
    pen_size: u32,
    shape_fill: &str,
) -> ShapeSpec {
    build_spec(kind, geom, pen_size, shape_fill)
}
