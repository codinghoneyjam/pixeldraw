//! Single error type for the whole engine (docs/contract.md §3).
//!
//! **Catalog drift note.** `docs/contract.md` §3 lists 19 codes, but
//! `src/core/errors.js` ships **21**: `VIEWPORT_INVALID` and
//! `VIEWPORT_OUT_OF_RANGE` were added for `features/document/document.js`
// viewport validation without the contract table being updated. The code is the
//! truth here (nothing pins the doc's count), so this port carries all 21 and
//! [`catalog`] pins the list so it cannot drift silently again.

use std::fmt;

/// The frozen catalog, in the JS source's declaration order.
pub const CODE_SCHEMA: &str = "SCHEMA";
pub const CODE_DUP_LAYER_ID: &str = "DUP_LAYER_ID";
pub const CODE_ACTIVE_LAYER_MISSING: &str = "ACTIVE_LAYER_MISSING";
pub const CODE_CHUNK_DUPLICATE: &str = "CHUNK_DUPLICATE";
pub const CODE_CHUNK_OUT_OF_CANVAS: &str = "CHUNK_OUT_OF_CANVAS";
pub const CODE_CHUNK_BAD_PNG: &str = "CHUNK_BAD_PNG";
pub const CODE_UNSUPPORTED_LAYER_TYPE: &str = "UNSUPPORTED_LAYER_TYPE";
pub const CODE_PNG_UNSUPPORTED: &str = "PNG_UNSUPPORTED";
pub const CODE_PNG_CRC: &str = "PNG_CRC";
pub const CODE_PNG_SIGNATURE: &str = "PNG_SIGNATURE";
pub const CODE_FILE_TOO_LARGE: &str = "FILE_TOO_LARGE";
pub const CODE_LAYER_NOT_FOUND: &str = "LAYER_NOT_FOUND";
pub const CODE_LAYER_LOCKED: &str = "LAYER_LOCKED";
pub const CODE_LAYER_HIDDEN: &str = "LAYER_HIDDEN";
pub const CODE_LAYER_LIMIT: &str = "LAYER_LIMIT";
pub const CODE_CANVAS_SIZE_INVALID: &str = "CANVAS_SIZE_INVALID";
pub const CODE_VIEWPORT_INVALID: &str = "VIEWPORT_INVALID";
pub const CODE_VIEWPORT_OUT_OF_RANGE: &str = "VIEWPORT_OUT_OF_RANGE";
pub const CODE_OUT_OF_RANGE: &str = "OUT_OF_RANGE";
pub const CODE_INVALID_STATE: &str = "INVALID_STATE";
pub const CODE_STORAGE_QUOTA: &str = "STORAGE_QUOTA";

/// Every code the engine is allowed to raise, frozen.
pub const CATALOG: [&str; 21] = [
    CODE_SCHEMA,
    CODE_DUP_LAYER_ID,
    CODE_ACTIVE_LAYER_MISSING,
    CODE_CHUNK_DUPLICATE,
    CODE_CHUNK_OUT_OF_CANVAS,
    CODE_CHUNK_BAD_PNG,
    CODE_UNSUPPORTED_LAYER_TYPE,
    CODE_PNG_UNSUPPORTED,
    CODE_PNG_CRC,
    CODE_PNG_SIGNATURE,
    CODE_FILE_TOO_LARGE,
    CODE_LAYER_NOT_FOUND,
    CODE_LAYER_LOCKED,
    CODE_LAYER_HIDDEN,
    CODE_LAYER_LIMIT,
    CODE_CANVAS_SIZE_INVALID,
    CODE_VIEWPORT_INVALID,
    CODE_VIEWPORT_OUT_OF_RANGE,
    CODE_OUT_OF_RANGE,
    CODE_INVALID_STATE,
    CODE_STORAGE_QUOTA,
];

/// `DrawToolError(code, message, details?)` — the one exception type.
///
/// Maps to the JS class: `name` is always `"DrawToolError"`, `code` is one of
/// [`CATALOG`], and `details` is an optional machine-readable payload.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawToolError {
    pub code: &'static str,
    pub message: String,
    pub details: Option<String>,
}

impl DrawToolError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        debug_assert!(
            CATALOG.contains(&code),
            "error code {code} is outside the frozen catalog"
        );
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        code: &'static str,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        let mut e = Self::new(code, message);
        e.details = Some(details.into());
        e
    }

    // <META - ROLE : constructors for the sites that raised each code in the JS tree | L88-120>
    pub fn canvas_size_invalid(w: u32, h: u32) -> Self {
        Self::new(
            CODE_CANVAS_SIZE_INVALID,
            format!("invalid canvas size {w}x{h}"),
        )
    }

    pub fn chunk_out_of_canvas(cx: u32, cy: u32) -> Self {
        Self::new(
            CODE_CHUNK_OUT_OF_CANVAS,
            format!("chunk ({cx},{cy}) out of canvas"),
        )
    }

    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::new(CODE_INVALID_STATE, message)
    }

    pub fn out_of_range(message: impl Into<String>) -> Self {
        Self::new(CODE_OUT_OF_RANGE, message)
    }

    pub fn layer_not_found(id: &str) -> Self {
        Self::with_details(CODE_LAYER_NOT_FOUND, format!("layer {id} not found"), id)
    }

    pub fn layer_locked() -> Self {
        Self::new(CODE_LAYER_LOCKED, "layer is locked")
    }

    pub fn layer_hidden() -> Self {
        Self::new(CODE_LAYER_HIDDEN, "layer is hidden")
    }

    pub fn png_unsupported(message: impl Into<String>) -> Self {
        Self::new(CODE_PNG_UNSUPPORTED, message)
    }
}

impl fmt::Display for DrawToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DrawToolError({}): {}", self.code, self.message)
    }
}

impl std::error::Error for DrawToolError {}

/// Engine-wide result alias.
pub type EngineResult<T> = Result<T, DrawToolError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_no_duplicates() {
        let mut sorted = CATALOG.to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), before, "duplicate code in the catalog");
    }

    #[test]
    fn every_constructor_uses_a_catalog_code() {
        let made = [
            DrawToolError::canvas_size_invalid(31, 32),
            DrawToolError::chunk_out_of_canvas(9, 9),
            DrawToolError::invalid_state("x"),
            DrawToolError::out_of_range("x"),
            DrawToolError::layer_not_found("layer-0001"),
            DrawToolError::layer_locked(),
            DrawToolError::layer_hidden(),
            DrawToolError::png_unsupported("x"),
        ];
        for e in made {
            assert!(CATALOG.contains(&e.code), "{} missing from catalog", e.code);
        }
    }

    #[test]
    fn display_matches_js_shape() {
        let e = DrawToolError::out_of_range("pen size 99 out of range");
        assert_eq!(
            e.to_string(),
            "DrawToolError(OUT_OF_RANGE): pen size 99 out of range"
        );
    }
}
