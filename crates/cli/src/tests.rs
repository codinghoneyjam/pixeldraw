//! CLI regression tests for the atlas planner and edit patches.

use pixel_engine::atlas::{
    ATLAS_MAX_CELLS, ATLAS_META_VERSION, AtlasCellSpec, AtlasLayout, AtlasMode,
};
use pixel_engine::io::serialize::{document_to_json, json_to_document};

use crate::atlas::{AtlasOptions, atlas_command, parse_cell, plan};
use crate::edit::apply_edit_patch;

#[test]
fn cell_parser_accepts_single_and_pair_sizes() {
    assert_eq!(parse_cell("512").unwrap(), (512, 512));
    assert_eq!(parse_cell("128x64").unwrap(), (128, 64));
    assert!(parse_cell("0x64").is_err());
    assert!(parse_cell("wide").is_err());
}

#[test]
fn atlas_options_default_to_preset_on() {
    let opts = AtlasOptions::parse(&[]).unwrap();
    assert!(opts.preset_all);
    assert!(opts.slots.is_empty());
    assert!(opts.mode.is_none());
}

#[test]
fn atlas_options_parse_slots_in_order() {
    let opts = AtlasOptions::parse(&[
        "--mode".into(),
        "composed".into(),
        "--slot".into(),
        "Both=Layer 1,Top".into(),
        "--no-preset".into(),
    ])
    .unwrap();
    assert_eq!(opts.mode, Some(AtlasMode::Composed));
    assert!(!opts.preset_all);
    assert_eq!(opts.slots[0].0, "Both");
    assert_eq!(opts.slots[0].1, vec!["Layer 1", "Top"]);
}

#[test]
fn a_missing_slot_separator_is_an_error() {
    assert!(AtlasOptions::parse(&["--slot".into(), "nope".into()]).is_err());
}

#[test]
fn plan_uses_the_512_default_and_near_square_columns() {
    let doc = pixel_engine::model::new_document(
        pixel_engine::model::NewDocumentOptions::default().size(2048, 1088),
    )
    .unwrap();
    let opts = AtlasOptions::parse(&[]).unwrap();
    let (layout, meta) = plan(&doc, &opts).unwrap();
    assert_eq!((layout.cell_w, layout.cell_h), (512, 512));
    assert_eq!(layout.columns, 1);
    assert_eq!((meta.sheet.w, meta.sheet.h), (512, 512));
    assert_eq!(meta.slots.len(), 1);
    assert_eq!(meta.slots[0].name, "Layer 1");
}

#[test]
fn composed_plan_preset_precedes_user_slots() {
    let doc = pixel_engine::model::new_document(
        pixel_engine::model::NewDocumentOptions::default().size(64, 64),
    )
    .unwrap();
    let opts = AtlasOptions::parse(&[
        "--mode".into(),
        "composed".into(),
        "--slot".into(),
        "Base=layer-0001".into(),
    ])
    .unwrap();
    let (layout, meta) = plan(&doc, &opts).unwrap();
    assert_eq!(layout.mode, AtlasMode::Composed);
    assert!(meta.preset_all);
    assert_eq!(meta.slots.len(), 2);
    assert_eq!(meta.slots[0].name, "All Layers");
    assert_eq!(meta.slots[1].name, "Base");
}

#[test]
fn spec_exposes_the_atlas_budget() {
    let value = atlas_command(&["spec".into()]).unwrap();
    assert_eq!(value["max_cells"], ATLAS_MAX_CELLS);
    assert_eq!(value["default_cell_max"], 512);
    assert_eq!(value["metadata_version"], ATLAS_META_VERSION);
}

#[test]
fn edit_patch_changes_integer_pixels_and_shapes_and_survives_document_roundtrip() {
    let mut doc = pixel_engine::model::new_document(
        pixel_engine::model::NewDocumentOptions::default().size(64, 64),
    )
    .unwrap();
    let layer_id = doc.active_layer_id.clone();
    let patch = serde_json::json!({"operations":[
        {"op":"set_pixel","layer":layer_id,"x":4,"y":5,"rgba":[255,0,32,255]},
        {"op":"draw_shape","layer":layer_id,"kind":"line","p0":[10,12],"p1":[14,12],"primary":[0,255,0,255]}
    ]});
    let summary = apply_edit_patch(&mut doc, &patch).unwrap();
    assert_eq!(summary.operations_applied, 2);
    assert_eq!(summary.pixel_changes[0]["pixels"], 6);
    assert_eq!(summary.pixel_changes[0]["bounds"]["x"], 4);
    assert_eq!(summary.pixel_changes[0]["bounds"]["y"], 5);
    assert_eq!(summary.pixel_changes[0]["bounds"]["width"], 11);
    assert_eq!(summary.pixel_changes[0]["bounds"]["height"], 8);
    assert_eq!(
        doc.layers[0].store.get_pixel(4, 5),
        u32::from_le_bytes([255, 0, 32, 255])
    );
    for x in 10..=14 {
        assert_eq!(
            doc.layers[0].store.get_pixel(x, 12),
            u32::from_le_bytes([0, 255, 0, 255])
        );
    }
    let saved = document_to_json(&doc).unwrap();
    let (loaded, warnings) = json_to_document(&saved).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(
        loaded.layers[0].store.get_pixel(4, 5),
        doc.layers[0].store.get_pixel(4, 5)
    );
    assert_eq!(
        loaded.layers[0].store.get_pixel(14, 12),
        doc.layers[0].store.get_pixel(14, 12)
    );
}

#[test]
fn edit_patch_persists_and_validates_atlas_layout() {
    let mut doc = pixel_engine::model::new_document(
        pixel_engine::model::NewDocumentOptions::default().size(64, 64),
    )
    .unwrap();
    let mut layout = AtlasLayout::from_document(&doc);
    layout.mode = AtlasMode::Composed;
    layout.columns = 2;
    layout.preset_all = false;
    layout.slots = vec![AtlasCellSpec::single("Idle", doc.active_layer_id.clone())];
    let patch = serde_json::json!({"operations":[
        {"op":"set_atlas","layout":layout}
    ]});
    apply_edit_patch(&mut doc, &patch).unwrap();
    let saved = document_to_json(&doc).unwrap();
    let (loaded, warnings) = json_to_document(&saved).unwrap();
    assert!(warnings.is_empty());
    let restored = loaded.atlas.expect("atlas layout persisted");
    assert_eq!(restored.mode, AtlasMode::Composed);
    assert_eq!(restored.columns, 2);
    assert_eq!(restored.slots[0].name, "Idle");
}

#[test]
fn failed_edit_patch_is_atomic_and_identifies_the_operation() {
    let mut doc = pixel_engine::model::new_document(
        pixel_engine::model::NewDocumentOptions::default().size(32, 32),
    )
    .unwrap();
    let patch = serde_json::json!({"operations":[
        {"op":"set_pixel","layer":"Layer 1","x":1,"y":1,"rgba":[12,34,56,255]},
        {"op":"set_pixel","layer":"Layer 1","x":99,"y":1,"rgba":[12,34,56,255]}
    ]});
    let error = apply_edit_patch(&mut doc, &patch).unwrap_err();
    assert!(error.message.contains("operation 1"));
    assert_eq!(doc.layers[0].store.get_pixel(1, 1), 0);
}
