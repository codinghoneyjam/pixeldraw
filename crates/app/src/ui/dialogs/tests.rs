//! Regression tests for dialog construction defaults.

use super::*;

#[test]
fn dialog_creation() {
    let d = Dialog::new_document();
    assert_eq!(d.new_doc_width, 512);
    assert_eq!(d.new_doc_background, "transparent");
}

#[test]
fn dialog_resize_uses_current_size() {
    let d = Dialog::resize_canvas(800, 600);
    assert_eq!(d.resize_width, 800);
    assert_eq!(d.resize_height, 600);
}
