//! New-document / resize-canvas validation.
//!
//! Both are pure `(value) -> (bool, message)` in the JS port; keeping them out
//! of the widget file means the bounds are testable without a window.
use super::super::Dialog;

/// Tiles -> px, the same `round(tiles * 64)` the dialog shows.
pub(crate) fn new_document_px(dialog: &Dialog) -> (u32, u32) {
    (
        (dialog.new_doc_w_tiles * 64.0).round().clamp(1.0, 4096.0) as u32,
        (dialog.new_doc_h_tiles * 64.0).round().clamp(1.0, 4096.0) as u32,
    )
}

pub(crate) fn new_document_valid(dialog: &Dialog) -> (bool, String) {
    let (w_px, h_px) = new_document_px(dialog);
    let ok = (0.5..=32.0).contains(&dialog.new_doc_w_tiles)
        && (0.5..=17.0).contains(&dialog.new_doc_h_tiles)
        && (32..=2048).contains(&w_px)
        && (32..=1088).contains(&h_px)
        && w_px % 32 == 0
        && h_px % 32 == 0;
    (
        ok,
        "범위: 너비 0.5–32, 높이 0.5–17 타일 (32px 배수 결과 필요)".to_string(),
    )
}

/// JS `showResizeCanvasDialog` sync: integers in 32..max, 32px multiple.
pub(crate) fn resize_valid(dialog: &Dialog) -> (bool, String) {
    let (w, h) = (dialog.resize_width, dialog.resize_height);
    let ok = (32..=2048).contains(&w) && (32..=1088).contains(&h) && w % 32 == 0 && h % 32 == 0;
    (
        ok,
        "범위: 32–2048 × 32–1088, 32의 배수여야 합니다".to_string(),
    )
}
