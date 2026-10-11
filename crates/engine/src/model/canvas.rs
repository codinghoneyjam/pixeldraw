//! The canvas triple plus its optional export viewport.
//!
//! Ported 1:1 from `src/features/document/document.js`. The stored canvas width
//! and height must stay 32-multiples so `ChunkStore` / `chunkKey` /
//! `PixelWriter` keep their alignment invariant; a viewport is how an arbitrary
//! export size is expressed without disturbing that. Absent means "export the
//! whole canvas".

use crate::constants::{DEFAULT_DOC_BACKGROUND, DEFAULT_DOC_HEIGHT_PX, DEFAULT_DOC_WIDTH_PX};
use crate::errors::{
    CODE_VIEWPORT_INVALID, CODE_VIEWPORT_OUT_OF_RANGE, DrawToolError, EngineResult,
};
use crate::pixel::parse_hex;

// <META - ROLE : validateBackground from document.js L31-37 | L31-37>
/// `"transparent"` or any parseable hex; anything else is `INVALID_STATE`.
pub fn validate_background(v: &str) -> EngineResult<()> {
    if v == "transparent" {
        return Ok(());
    }
    match parse_hex(v) {
        Some(_) => Ok(()),
        None => Err(DrawToolError::invalid_state(
            "background must be transparent or hex color",
        )),
    }
}

/// A logical export viewport: `[x, y, w, h]` inside the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

// <META - ROLE : validateViewport from document.js L50-71 | L50-71>
/// Validate an optional viewport against its canvas.
///
/// **Two JS guards are absorbed by the type here**, the same way
/// `composite.rs` absorbed `assertDoc`/`assertOut`: "viewport must be an object"
/// (a non-object or an array) and "viewport k must be an integer" cannot be
/// expressed once `Viewport` is a struct of `u32`. What is left is the part that
/// changes the answer — the size and range checks, with the same codes.
pub fn validate_viewport(
    v: Option<Viewport>,
    canvas_w: u32,
    canvas_h: u32,
) -> EngineResult<Option<Viewport>> {
    let Some(vp) = v else { return Ok(None) };
    if vp.w < 1 || vp.h < 1 {
        return Err(DrawToolError::new(
            CODE_VIEWPORT_INVALID,
            format!("viewport size must be positive, got {}x{}", vp.w, vp.h),
        ));
    }
    // u64 so a large viewport cannot wrap past the canvas edge.
    let (x, y, w, h) = (vp.x as u64, vp.y as u64, vp.w as u64, vp.h as u64);
    if x + w > canvas_w as u64 || y + h > canvas_h as u64 {
        return Err(DrawToolError::new(
            CODE_VIEWPORT_OUT_OF_RANGE,
            format!(
                "viewport {},{} {}x{} exceeds canvas {}x{}",
                vp.x, vp.y, vp.w, vp.h, canvas_w, canvas_h
            ),
        ));
    }
    Ok(Some(vp))
}

/// The canvas triple plus its optional export viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasInfo {
    pub width_px: u32,
    pub height_px: u32,
    pub background: String,
    pub viewport: Option<Viewport>,
}

impl Default for CanvasInfo {
    fn default() -> Self {
        Self {
            width_px: DEFAULT_DOC_WIDTH_PX,
            height_px: DEFAULT_DOC_HEIGHT_PX,
            background: DEFAULT_DOC_BACKGROUND.to_string(),
            viewport: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // <META - ROLE : parity with document.js L32-37 background guard | L32-37>
    #[test]
    fn background_guard() {
        assert!(validate_background("transparent").is_ok());
        assert!(validate_background("#ff0000").is_ok());
        assert_eq!(
            validate_background("nope").unwrap_err().code,
            crate::errors::CODE_INVALID_STATE
        );
    }

    // <META - ROLE : parity with document.js L50-71 viewport range cases | L50-71>
    #[test]
    fn viewport_ranges_against_the_canvas() {
        let ok = Viewport {
            x: 0,
            y: 0,
            w: 32,
            h: 32,
        };
        assert_eq!(validate_viewport(Some(ok), 32, 32).unwrap(), Some(ok));
        let edge = Viewport {
            x: 16,
            y: 0,
            w: 16,
            h: 32,
        };
        assert!(validate_viewport(Some(edge), 32, 32).is_ok());
        // A zero-sized viewport is invalid, not out of range.
        let empty = Viewport {
            x: 0,
            y: 0,
            w: 0,
            h: 32,
        };
        assert_eq!(
            validate_viewport(Some(empty), 32, 32).unwrap_err().code,
            CODE_VIEWPORT_INVALID
        );
        let over = Viewport {
            x: 16,
            y: 0,
            w: 17,
            h: 32,
        };
        assert_eq!(
            validate_viewport(Some(over), 32, 32).unwrap_err().code,
            CODE_VIEWPORT_OUT_OF_RANGE
        );
        // x + w wraps in u32 at 2^31; u64 keeps it a range error.
        let huge = Viewport {
            x: u32::MAX,
            y: 0,
            w: u32::MAX,
            h: 32,
        };
        assert_eq!(
            validate_viewport(Some(huge), 2048, 2048).unwrap_err().code,
            CODE_VIEWPORT_OUT_OF_RANGE
        );
        assert!(validate_viewport(None, 32, 32).unwrap().is_none());
    }

    #[test]
    fn canvas_defaults_are_the_frozen_document() {
        let c = CanvasInfo::default();
        assert_eq!(c.width_px, DEFAULT_DOC_WIDTH_PX);
        assert_eq!(c.height_px, DEFAULT_DOC_HEIGHT_PX);
        assert_eq!(c.background, DEFAULT_DOC_BACKGROUND);
        assert_eq!(c.viewport, None);
    }
}
