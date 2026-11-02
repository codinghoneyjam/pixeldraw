//! Menubar regression tests.

use super::*;

#[test]
fn menubar_creation_works() {
    let menubar = Menubar::new();
    assert_eq!(menubar.actions.label(&Action::NewDocument), "새 문서");
}

/// `index.html` L17: the File slot is the menu label. The port read the
/// first item's action label here, so the button said `새 문서`.
#[test]
fn the_file_slot_is_labelled_file() {
    assert_eq!(FILE_MENU_LABEL, "파일");
    assert_ne!(
        FILE_MENU_LABEL,
        ActionMap::new().label(&Action::NewDocument)
    );
}

#[test]
fn menu_blocks_have_stable_indices_labels_and_slots() {
    assert_eq!(MenuBlock::ALL.len(), 5);
    for (i, block) in MenuBlock::ALL.into_iter().enumerate() {
        assert_eq!(block.index(), i);
        assert!(!block.label().is_empty());
        assert_eq!(BLOCK_SLOT, egui::vec2(88.0, 26.0));
    }
    assert_eq!(MenuBlock::File.label(), FILE_MENU_LABEL);
    assert_eq!(MenuBlock::Help.label(), "도움말");
}

#[test]
fn hover_menu_closes_only_after_leaving_the_slot_and_popup() {
    let button = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(50.0, 20.0));
    let popup = Some(egui::Rect::from_min_size(
        egui::pos2(10.0, 34.0),
        egui::vec2(120.0, 150.0),
    ));
    assert!(menu_hover_keeps_open(egui::pos2(30.0, 18.0), button, popup));
    assert!(menu_hover_keeps_open(egui::pos2(40.0, 80.0), button, popup));
    // The gap between the slot and the popup is part of the hover path.
    assert!(menu_hover_keeps_open(egui::pos2(30.0, 32.0), button, popup));
    assert!(!menu_hover_keeps_open(
        egui::pos2(300.0, 300.0),
        button,
        popup
    ));
    assert!(!menu_hover_keeps_open(
        egui::pos2(300.0, 300.0),
        button,
        None
    ));
}
