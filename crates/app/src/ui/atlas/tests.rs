//! Regression tests for the atlas workbench state and slot actions.

use std::collections::BTreeSet;

use pixel_engine::atlas::{ATLAS_META_VERSION, AtlasCellSpec, AtlasMode};

use super::slots::{SlotAction, short_name};
use super::state::AtlasState;
use crate::file_ops::{export_atlas_bytes, new_document_with_bg, open_png_bytes};
use crate::session_glue::AppState;

#[test]
fn state_starts_in_layers_mode_with_the_first_cell_focused() {
    let mut app = AppState::new();
    app.session.add_layer(Some("Top")).unwrap();
    let state = AtlasState::from_app(&app);
    assert_eq!(state.layout.mode, AtlasMode::Layers);
    assert_eq!(state.layout.cell_w, 512);
    assert_eq!(state.focus, Some(0));
    let doc = app.session.require_doc().unwrap();
    assert_eq!(
        state
            .layout
            .cells(doc)
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Top", "Layer 1"]
    );
}

#[test]
fn cell_snap_keeps_sizes_on_the_32px_grid() {
    assert_eq!(AtlasState::snap_cell(1), 32);
    assert_eq!(AtlasState::snap_cell(33), 64);
    assert_eq!(AtlasState::snap_cell(512), 512);
    assert_eq!(AtlasState::snap_cell(8000), 4096);
}

#[test]
fn focus_is_clamped_after_cells_disappear() {
    let mut state = AtlasState {
        focus: Some(4),
        selected: BTreeSet::from([1, 4]),
        ..Default::default()
    };
    state.clamp_focus(2);
    assert_eq!(state.focus, Some(1));
    assert_eq!(state.selected, BTreeSet::from([1]));
}

#[test]
fn composed_preset_can_be_disabled_without_losing_slots() {
    let mut app = AppState::new();
    let top = app.session.add_layer(Some("Top")).unwrap();
    let mut state = AtlasState::from_app(&app);
    state.layout.mode = AtlasMode::Composed;
    state.layout.slots = vec![AtlasCellSpec::single("Top only", top)];
    let doc = app.session.require_doc().unwrap();
    assert_eq!(state.cells(doc).len(), 2);
    state.layout.preset_all = false;
    assert_eq!(state.cells(doc).len(), 1);
}

#[test]
fn atlas_layout_is_a_draft_until_committed() {
    let mut app = AppState::new();
    let mut state = AtlasState::from_app(&app);
    state.layout.columns = 2;

    // Editing the modal's draft does not dirty the document. Cancel simply
    // drops this state; only the explicit confirmation path calls commit.
    assert_eq!(app.session.require_doc().unwrap().atlas, None);
    state.commit_layout(&mut app).unwrap();
    assert_eq!(
        app.session
            .require_doc()
            .unwrap()
            .atlas
            .as_ref()
            .unwrap()
            .columns,
        2
    );
}

#[test]
fn focused_layer_cell_resolves_to_the_layer_and_commit_is_one_undo_step() {
    let mut app = AppState::new();
    let top_id = app.session.add_layer(Some("Top")).unwrap();
    let mut state = AtlasState::from_app(&app);
    let doc = app.session.require_doc().unwrap();

    assert_eq!(
        state.focused_layer_id(doc).as_deref(),
        Some(top_id.as_str())
    );
    state.focus = Some(1);
    assert_eq!(
        state.focused_layer_id(doc).as_deref(),
        Some(doc.layers[0].id.as_str())
    );

    state.layout.columns = 2;
    state.commit_layout(&mut app).unwrap();
    let persisted = app.session.require_doc().unwrap().atlas.clone();
    assert_eq!(persisted.as_ref().unwrap().columns, 2);
    assert!(app.session.undo().unwrap());
    assert_eq!(app.session.require_doc().unwrap().atlas, None);
    assert!(app.session.redo().unwrap());
    assert_eq!(app.session.require_doc().unwrap().atlas, persisted);
}

#[test]
fn composed_slot_names_must_be_unique_and_bounded() {
    let mut app = AppState::new();
    let mut state = AtlasState::from_app(&app);
    state.layout.mode = AtlasMode::Composed;
    state.layout.preset_all = false;
    state.layout.slots = vec![
        AtlasCellSpec::single("Idle", "layer-0001"),
        AtlasCellSpec::single("idle", "layer-0001"),
    ];
    assert!(state.commit_layout(&mut app).is_err());
    state.layout.slots[1].name = "x".repeat(65);
    assert!(state.commit_layout(&mut app).is_err());
}

#[test]
fn invalid_atlas_draft_cannot_be_committed() {
    let mut app = AppState::new();
    let mut state = AtlasState::from_app(&app);
    state.layout.cell_w = 0;

    assert!(state.commit_layout(&mut app).is_err());
    assert_eq!(app.session.require_doc().unwrap().atlas, None);
}

#[test]
fn duplicate_slot_keeps_layer_order_and_inserts_after_source() {
    let mut slots = vec![
        AtlasCellSpec::composed("Idle", vec!["base".into(), "eyes".into()]),
        AtlasCellSpec::single("Run", "base"),
    ];
    SlotAction::DuplicateSlot(0).apply(&mut slots, 3);
    assert_eq!(slots.len(), 3);
    assert_eq!(slots[1].name, "Idle copy");
    assert_eq!(slots[1].layer_ids, vec!["base", "eyes"]);
    assert!(slots[1].merged);
    assert_eq!(slots[2].name, "Run");
}

#[test]
fn duplicate_slot_respects_capacity_and_invalid_selection() {
    let mut slots = vec![AtlasCellSpec::single("Idle", "base")];
    SlotAction::DuplicateSlot(0).apply(&mut slots, 1);
    SlotAction::DuplicateSlot(8).apply(&mut slots, 4);
    assert_eq!(slots.len(), 1);
}

#[test]
fn slot_grid_swap_moves_the_whole_layer_stack() {
    let mut slots = vec![
        AtlasCellSpec::composed("Idle", vec!["base".into(), "eyes".into()]),
        AtlasCellSpec::single("Run", "legs"),
        AtlasCellSpec::single("Jump", "air"),
    ];
    SlotAction::SwapSlots(0, 2).apply(&mut slots, 3);
    assert_eq!(
        slots
            .iter()
            .map(|slot| slot.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Jump", "Run", "Idle"]
    );
    assert_eq!(slots[2].layer_ids, vec!["base", "eyes"]);
    assert!(slots[2].merged);
}

#[test]
fn layer_move_up_and_down_swaps_the_composition_order() {
    let mut slots = vec![AtlasCellSpec::composed(
        "Idle",
        vec!["base".into(), "eyes".into(), "glow".into()],
    )];
    // ▲ on the middle layer: draw it later (towards the top).
    SlotAction::MoveLayer(0, 1, 2).apply(&mut slots, 1);
    assert_eq!(slots[0].layer_ids, vec!["base", "glow", "eyes"]);
    // ▼ on the top layer: draw it earlier.
    SlotAction::MoveLayer(0, 2, 1).apply(&mut slots, 1);
    assert_eq!(slots[0].layer_ids, vec!["base", "eyes", "glow"]);
}

#[test]
fn layer_drag_moves_a_layer_to_the_dropped_position() {
    let mut slots = vec![AtlasCellSpec::composed(
        "Idle",
        vec!["base".into(), "eyes".into(), "glow".into()],
    )];
    // Drop the bottom layer above the top row: "to == len" means the new top.
    SlotAction::MoveLayerTo(0, 0, 3).apply(&mut slots, 1);
    assert_eq!(slots[0].layer_ids, vec!["eyes", "glow", "base"]);
    // Drop it back onto the bottom row: final index 0.
    SlotAction::MoveLayerTo(0, 2, 0).apply(&mut slots, 1);
    assert_eq!(slots[0].layer_ids, vec!["base", "eyes", "glow"]);
    // Drop between two rows keeps both neighbours.
    SlotAction::MoveLayerTo(0, 0, 1).apply(&mut slots, 1);
    assert_eq!(slots[0].layer_ids, vec!["eyes", "base", "glow"]);
}

#[test]
fn slot_tile_names_are_truncated_without_splitting_characters() {
    assert_eq!(short_name("Idle", 8), "Idle");
    assert_eq!(short_name("달리기 오른쪽", 5), "달리기 오…");
}

#[test]
fn composed_workspace_renders_without_panicking() {
    let mut app = AppState::new();
    let base = app.session.add_layer(Some("Base")).unwrap();
    let mut state = AtlasState::from_app(&app);
    state.layout.mode = AtlasMode::Composed;
    state.layout.columns = 2;
    state.layout.slots = vec![
        AtlasCellSpec::composed("Idle", vec![base.clone()]),
        AtlasCellSpec::single("Run", base),
    ];
    state.layout.preset_all = false;

    egui::__run_test_ui(|ui| {
        ui.set_min_width(1040.0);
        super::show(ui, &mut state, &mut app);
    });
}

#[test]
fn exported_sheet_reopens_as_a_cell_sized_document() {
    let mut app = AppState::new();
    new_document_with_bg(&mut app, 64, 64, "transparent").unwrap();
    let mut state = AtlasState::from_app(&app);
    state.layout.columns = 2;
    let bytes = export_atlas_bytes(&app, &state.layout).unwrap();
    let img = pixel_engine::io::png::decode_png_large(&bytes).unwrap();
    assert_eq!((img.width, img.height), (128, 64));
    let meta = pixel_engine::atlas::AtlasMetadata::from_json_bytes(
        img.metadata_chunk("dtAt").expect("dtAt"),
    )
    .unwrap();
    assert_eq!(meta.version, ATLAS_META_VERSION);
    open_png_bytes(&mut app, &bytes).unwrap();
    let doc = app.session.require_doc().unwrap();
    assert_eq!((doc.canvas.width_px, doc.canvas.height_px), (64, 64));
}
