//! Core constants — single source of truth (docs/contract.md §1).
//!
//! Ported 1:1 from `src/core/constants.js`. Values are frozen: the schema, the
//! serializers and the golden fixtures all depend on them, so nothing here may
//! be "cleaned up" without re-baking `tests/fixtures/raster_golden.json`.

use crate::errors::DrawToolError;

pub const TILE_PX: u32 = 64;
pub const UNIT_PX: u32 = 32;
pub const CHUNK_PX: u32 = 32;
pub const CHUNK_LEN: usize = 32 * 32 * 4;
pub const MIN_SIZE_PX: u32 = 32;
pub const MAX_W_PX: u32 = 2048;
pub const MAX_H_PX: u32 = 1088;
pub const MAX_CHUNKS_X: u32 = 64;
pub const MAX_CHUNKS_Y: u32 = 34;
pub const PEN_MIN: u32 = 1;
pub const PEN_MAX: u32 = 64;
pub const PEN_DEFAULT: u32 = 1;
pub const MAX_LAYERS: usize = 64;
pub const UNDO_MAX_STEPS: usize = 200;
pub const UNDO_MAX_BYTES: usize = 96 * 1024 * 1024;
pub const SCHEMA_VERSION: &str = "2.0.0";

/// Zoom ladder = CSS px per canvas px, 12 rungs, frozen.
pub const ZOOM_LEVELS: [f64; 12] = [
    0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0,
];

/// Ctrl+' cycle order for the grid overlay.
pub const GRID_MODES: [&str; 4] = ["off", "unit", "tile", "pixel"];

/// Frozen default document.
pub const DEFAULT_DOC_WIDTH_PX: u32 = 512;
pub const DEFAULT_DOC_HEIGHT_PX: u32 = 512;
pub const DEFAULT_DOC_BACKGROUND: &str = "transparent";

// --- Phase 2 display constants (docs/contract.md §1-2) -----------------------
//

/// One frame's budget for painting dirty chunks that are *off* screen
/// (`src/render/renderer_composite.js` L5).
pub const FRAME_BUDGET_MS: u64 = 6;

/// Checkerboard colours, screen-fixed (`src/render/background.js` L6-7).
pub const CHECKER_RGBA_A: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
pub const CHECKER_RGBA_B: [u8; 4] = [0xd9, 0xd9, 0xd9, 0xff];

/// Checkerboard cell size in CSS px; the device size is `max(1, rnd(this*dpr))`.
pub const CHECKER_CSS_PX: f64 = 8.0;

/// Canvas border, `rgba(0,0,0,0.6)` (`src/render/renderer_display.js` L4).
pub const BORDER_RGBA: [u8; 4] = [0x00, 0x00, 0x00, 0x99];

/// Grid stroke alphas — they double as the palette marker that selects a
/// stroke's weight, so they are identity, not just weights (`grid.js` L14-16).
pub const GRID_ALPHA_THIN: f64 = 0.3;
pub const GRID_ALPHA_THICK: f64 = 0.55;
pub const GRID_ALPHA_PIXEL: f64 = 0.15;

/// Grid stroke widths in CSS px, converted with **ceil** to device px (`grid.js` L19-21).
pub const GRID_THIN_CSS_PX: f64 = 1.0;
pub const GRID_THICK_CSS_PX: f64 = 2.0;

/// Dark bevel run width on a light run's trailing edge (device px).
pub const GRID_BEVEL_DEV_PX: u32 = 1;

/// Per-bevel-pass ink (`grid.js` L48-51). Thick strokes out-ink thin ones.
pub const GRID_PASS_ALPHA_THICK_LIGHT: f64 = 0.44;
pub const GRID_PASS_ALPHA_THICK_DARK: f64 = 0.5;
pub const GRID_PASS_ALPHA_THIN_LIGHT: f64 = 0.3;
pub const GRID_PASS_ALPHA_THIN_DARK: f64 = 0.32;

/// `cy * MAX_CHUNKS_X + cx` — the sparse-map key rule (docs/contract.md §1).
///
/// `chunkKey(1,2) === 129` in the JS tree; keep that identity.
#[inline]
pub const fn chunk_key(cx: u32, cy: u32) -> u64 {
    (cy as u64) * (MAX_CHUNKS_X as u64) + cx as u64
}

/// Inverse of [`chunk_key`].
#[inline]
pub const fn chunk_coords(key: u64) -> (u32, u32) {
    (
        (key % MAX_CHUNKS_X as u64) as u32,
        (key / MAX_CHUNKS_X as u64) as u32,
    )
}

/// 32 or above, within the caps, and a multiple of `CHUNK_PX`.
#[inline]
pub fn is_valid_canvas_size(w: u32, h: u32) -> bool {
    w >= MIN_SIZE_PX
        && h >= MIN_SIZE_PX
        && w <= MAX_W_PX
        && h <= MAX_H_PX
        && w.is_multiple_of(CHUNK_PX)
        && h.is_multiple_of(CHUNK_PX)
}

/// Canvas size guard that raises the JS tree's error code.
pub fn require_valid_canvas_size(w: u32, h: u32) -> Result<(), DrawToolError> {
    if is_valid_canvas_size(w, h) {
        Ok(())
    } else {
        Err(DrawToolError::canvas_size_invalid(w, h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // <META - ROLE : the frozen table, mirrored from tests/core_basic.test.mjs | L40-58>
    #[test]
    fn table_values() {
        assert_eq!(TILE_PX, 64);
        assert_eq!(UNIT_PX, 32);
        assert_eq!(CHUNK_PX, 32);
        assert_eq!(CHUNK_LEN, 4096);
        assert_eq!(MIN_SIZE_PX, 32);
        assert_eq!(MAX_W_PX, 2048);
        assert_eq!(MAX_H_PX, 1088);
        assert_eq!(MAX_CHUNKS_X, 64);
        assert_eq!(MAX_CHUNKS_Y, 34);
        assert_eq!(PEN_MIN, 1);
        assert_eq!(PEN_MAX, 64);
        assert_eq!(PEN_DEFAULT, 1);
        assert_eq!(MAX_LAYERS, 64);
        assert_eq!(UNDO_MAX_STEPS, 200);
        assert_eq!(UNDO_MAX_BYTES, 96 * 1024 * 1024);
        assert_eq!(SCHEMA_VERSION, "2.0.0");
    }

    #[test]
    fn zoom_ladder_is_frozen() {
        assert_eq!(
            ZOOM_LEVELS,
            [
                0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0
            ]
        );
    }

    #[test]
    fn chunk_key_identity() {
        assert_eq!(chunk_key(1, 2), 129);
        assert_eq!(chunk_coords(129), (1, 2));
        // round trip over the whole sparse grid
        for key in 0..(MAX_CHUNKS_X as u64 * MAX_CHUNKS_Y as u64) {
            let (cx, cy) = chunk_coords(key);
            assert_eq!(chunk_key(cx, cy), key);
        }
    }

    #[test]
    fn canvas_size_bounds() {
        assert!(!is_valid_canvas_size(31, 32));
        assert!(!is_valid_canvas_size(33, 32));
        assert!(!is_valid_canvas_size(2080, 32));
        assert!(!is_valid_canvas_size(32, 1120));
        assert!(is_valid_canvas_size(32, 32));
        assert!(is_valid_canvas_size(1920, 1088));
        assert!(is_valid_canvas_size(512, 512));
        assert!(is_valid_canvas_size(2048, 128));
        assert!(!is_valid_canvas_size(2048 + 32, 128));
    }

    // Phase 2 display constants, pinned against the JS sources they came from.
    #[test]
    fn display_constants_are_frozen() {
        assert_eq!(FRAME_BUDGET_MS, 6);
        assert_eq!(CHECKER_RGBA_A, [0xff, 0xff, 0xff, 0xff]);
        assert_eq!(CHECKER_RGBA_B, [0xd9, 0xd9, 0xd9, 0xff]);
        assert_eq!(CHECKER_CSS_PX, 8.0);
        assert_eq!(BORDER_RGBA, [0, 0, 0, 0x99]);
        assert_eq!(GRID_ALPHA_THIN, 0.3);
        assert_eq!(GRID_ALPHA_THICK, 0.55);
        assert_eq!(GRID_ALPHA_PIXEL, 0.15);
        assert_eq!(GRID_THIN_CSS_PX, 1.0);
        assert_eq!(GRID_THICK_CSS_PX, 2.0);
        assert_eq!(GRID_BEVEL_DEV_PX, 1);
        assert_eq!(GRID_PASS_ALPHA_THICK_LIGHT, 0.44);
        assert_eq!(GRID_PASS_ALPHA_THICK_DARK, 0.5);
        assert_eq!(GRID_PASS_ALPHA_THIN_LIGHT, 0.3);
        assert_eq!(GRID_PASS_ALPHA_THIN_DARK, 0.32);
    }
}
