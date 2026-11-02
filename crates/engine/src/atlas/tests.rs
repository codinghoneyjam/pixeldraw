use super::*;
use crate::chunkstore::PixelWriter;
use crate::io::png::{decode_png_large, encode_png_large};
use crate::model::layer::Layer;
use crate::model::{NewDocumentOptions, new_document};
use crate::pixel::pack_rgba;

const RED: [u8; 4] = [0xff, 0, 0, 0xff];
const BLUE: [u8; 4] = [0, 0, 0xff, 0xff];

fn packed(c: [u8; 4]) -> u32 {
    pack_rgba(c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64)
}

fn dotted(name: &str, id: &str, size: u32, x: u32, y: u32, colour: [u8; 4]) -> Layer {
    let mut layer = Layer::create(id, name, size, size).unwrap();
    {
        let mut writer = PixelWriter::new(&mut layer.store, id);
        writer.set(x, y, packed(colour));
    }
    layer
}

/// 64×64 document with `bottom` red at (1,1) and `top` blue at (20,20).
fn two_layer_doc() -> Document {
    let mut doc = new_document(NewDocumentOptions::default().size(64, 64)).unwrap();
    doc.layers[0] = dotted("Layer 1", "layer-1", 64, 1, 1, RED);
    doc.active_layer_id = "layer-1".into();
    let top = dotted("Top", "layer-top", 64, 20, 20, BLUE);
    doc.layers.push(top);
    doc.active_layer_id = "layer-top".into();
    doc
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * width + x) * 4) as usize;
    [rgba[o], rgba[o + 1], rgba[o + 2], rgba[o + 3]]
}

#[test]
fn default_cell_size_clamps_each_axis_to_512() {
    assert_eq!(default_cell_size(512, 512), (512, 512));
    assert_eq!(default_cell_size(1024, 512), (512, 512));
    assert_eq!(default_cell_size(2048, 1088), (512, 512));
    assert_eq!(default_cell_size(2048, 256), (512, 256));
    // Upscaling is not the atlas default: small canvases keep their size.
    assert_eq!(default_cell_size(64, 32), (64, 32));
    assert_eq!(default_cell_size(0, 0), (1, 1));
}

#[test]
fn column_ladder_covers_the_full_64_cell_range() {
    for count in 1..=64usize {
        let cols = best_columns(count);
        assert!((1..=count as u32).contains(&cols));
        let rows = (count as u32).div_ceil(cols);
        assert!(cols * rows <= 64);
    }
    assert_eq!(best_columns(1), 1);
    assert_eq!(best_columns(16), 4);
    assert_eq!(best_columns(64), 8);
}

#[test]
fn layers_mode_is_top_first_like_the_layer_panel() {
    let doc = two_layer_doc();
    let layout = AtlasLayout::from_document(&doc);
    let cells = layout.cells(&doc);
    assert_eq!(
        cells.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        vec!["Top", "Layer 1"]
    );
    assert!(!cells[0].merged);
}

#[test]
fn sheet_build_reports_progress_and_can_be_cancelled_between_cells() {
    let doc = two_layer_doc();
    let mut layout = AtlasLayout::from_document(&doc);
    layout.columns = 2;
    let mut updates = Vec::new();
    let result = build_sheet_with_progress(&doc, &layout, |done, total| {
        updates.push((done, total));
        done < 1
    });

    assert!(result.is_err());
    assert_eq!(updates, vec![(0, 2), (1, 2)]);
}

#[test]
fn composed_preset_is_first_and_then_user_slots() {
    let doc = two_layer_doc();
    let mut layout = AtlasLayout::from_document(&doc);
    layout.mode = AtlasMode::Composed;
    layout.slots = vec![AtlasCellSpec::single("Only top", "layer-top")];
    let cells = layout.cells(&doc);
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[0].name, "All Layers");
    assert_eq!(cells[0].layer_ids, vec!["layer-1", "layer-top"]);
    assert_eq!(cells[1].name, "Only top");

    layout.preset_all = false;
    let cells = layout.cells(&doc);
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].name, "Only top");
}

#[test]
fn ordered_composition_paints_the_last_layer_on_top() {
    let doc = two_layer_doc();
    let cell = AtlasCellSpec::composed("Both", vec!["layer-1".into(), "layer-top".into()]);
    let top = AtlasCellSpec::composed("Both", vec!["layer-top".into(), "layer-1".into()]);
    let a = build_cell_rgba(&doc, &cell, 64, 64).unwrap();
    let b = build_cell_rgba(&doc, &top, 64, 64).unwrap();
    // Each dot is only in its own source, so both cells show both dots.
    assert_eq!(pixel(&a.rgba, 64, 1, 1), RED);
    assert_eq!(pixel(&a.rgba, 64, 20, 20), BLUE);
    assert_eq!(pixel(&b.rgba, 64, 1, 1), RED);
    // Order shows up where the dots overlap: none here, so add an overlap.
    let mut doc = doc;
    let mut extra = dotted("Middle", "layer-mid", 64, 1, 1, BLUE);
    extra.visible = true;
    doc.layers.insert(1, extra);
    let red_over = AtlasCellSpec::composed("x", vec!["layer-mid".into(), "layer-1".into()]);
    let blue_over = AtlasCellSpec::composed("x", vec!["layer-1".into(), "layer-mid".into()]);
    let red = build_cell_rgba(&doc, &red_over, 64, 64).unwrap();
    let blue = build_cell_rgba(&doc, &blue_over, 64, 64).unwrap();
    assert_eq!(pixel(&red.rgba, 64, 1, 1), RED);
    assert_eq!(pixel(&blue.rgba, 64, 1, 1), BLUE);
}

#[test]
fn cell_resampling_keeps_aspect_and_centres_the_image() {
    let doc = two_layer_doc();
    let cell = AtlasCellSpec::single("Top", "layer-top");
    // 64×64 into 96×64: the content is centred with a 16px transparent band.
    let out = build_cell_rgba(&doc, &cell, 96, 64).unwrap();
    assert_eq!(pixel(&out.rgba, 96, 0, 0), [0, 0, 0, 0]);
    assert_eq!(pixel(&out.rgba, 96, 36, 20), BLUE);
}

#[test]
fn sheet_places_cells_row_major() {
    let doc = two_layer_doc();
    let mut layout = AtlasLayout::from_document(&doc);
    layout.columns = 2;
    layout.cell_w = 64;
    layout.cell_h = 64;
    let sheet = build_sheet(&doc, &layout).unwrap();
    assert_eq!((sheet.width, sheet.height), (128, 64));
    // Cell 1 (top layer) has blue at (20,20), cell 2 has red at (65,1).
    assert_eq!(pixel(&sheet.rgba, 128, 20, 20), BLUE);
    assert_eq!(pixel(&sheet.rgba, 128, 64 + 1, 1), RED);
    assert_eq!(sheet.metadata.mode, AtlasMode::Layers);
    assert_eq!(sheet.metadata.grid.cols, 2);
    assert_eq!(sheet.metadata.slots.len(), 2);
    assert_eq!(sheet.metadata.slots[0].layer_ids, vec!["layer-top"]);
}

#[test]
fn png_export_carries_v3_metadata() {
    let doc = two_layer_doc();
    let mut layout = AtlasLayout::from_document(&doc);
    layout.mode = AtlasMode::Composed;
    layout.columns = 1;
    let export = export_atlas_png(&doc, &layout).unwrap();
    let img = decode_png_large(&export.bytes).unwrap();
    assert_eq!((img.width, img.height), (64, 64));
    let raw = img.metadata_chunk("dtAt").expect("dtAt chunk");
    let meta = AtlasMetadata::from_json_bytes(raw).unwrap();
    assert_eq!(meta.version, 3);
    assert_eq!(meta.mode, AtlasMode::Composed);
    assert!(meta.preset_all);
    assert_eq!(meta.slots[0].layer_ids, vec!["layer-1", "layer-top"]);
    assert_eq!(meta.slots[0].sources, vec!["Layer 1", "Top"]);
}

#[test]
fn spaced_sheet_places_cells_at_margin_and_padding_offsets() {
    let doc = two_layer_doc();
    let mut layout = AtlasLayout::from_document(&doc);
    layout.columns = 2;
    layout.cell_w = 64;
    layout.cell_h = 64;
    layout.margin = 3;
    layout.padding = 5;

    let sheet = build_sheet(&doc, &layout).unwrap();
    assert_eq!((sheet.width, sheet.height), (139, 70));
    assert_eq!(pixel(&sheet.rgba, sheet.width, 23, 23), BLUE);
    assert_eq!(pixel(&sheet.rgba, sheet.width, 73, 4), RED);
    assert_eq!(pixel(&sheet.rgba, sheet.width, 2, 3), [0, 0, 0, 0]);
    assert_eq!(pixel(&sheet.rgba, sheet.width, 68, 4), [0, 0, 0, 0]);
    assert_eq!(sheet.metadata.margin, 3);
    assert_eq!(sheet.metadata.padding, 5);
    assert!(sheet.metadata.sheet_matches_grid());
}

#[test]
fn layout_rejects_more_than_sixty_four_cells_and_unknown_layers() {
    let doc = two_layer_doc();
    let mut layout = AtlasLayout::from_document(&doc);
    layout.mode = AtlasMode::Composed;
    layout.preset_all = false;
    layout.columns = 65;
    assert!(layout.validate(&doc).is_err());
    layout.columns = 1;
    layout.slots = vec![AtlasCellSpec::single("missing", "nope")];
    assert!(layout.validate(&doc).is_err());
    layout.slots = vec![AtlasCellSpec::single("ok", "layer-top"); 65];
    assert!(layout.validate(&doc).is_err());
}

#[test]
fn sixty_four_composed_cells_fit_the_layer_and_pixel_budgets() {
    let doc = two_layer_doc();
    let mut layout = AtlasLayout::from_document(&doc);
    layout.mode = AtlasMode::Composed;
    layout.preset_all = false;
    layout.columns = 8;
    layout.rows = Some(8);
    layout.slots = (0..64)
        .map(|i| AtlasCellSpec::single(format!("Frame {i:02}"), "layer-top"))
        .collect();
    layout.validate(&doc).unwrap();
    let sheet = build_sheet(&doc, &layout).unwrap();
    assert_eq!((sheet.width, sheet.height), (512, 512));
    assert_eq!(sheet.metadata.slots.len(), 64);
}

#[test]
fn atlas_png_can_exceed_the_editor_document_caps() {
    // 2048×2048 is a legal atlas sheet (4.2 MP) but larger than the editor's
    // 2048×1088 document budget. The large codec must accept it.
    let rgba = vec![0u8; 2048 * 2048 * 4];
    let bytes = encode_png_large(&rgba, 2048, 2048).unwrap();
    let img = decode_png_large(&bytes).unwrap();
    assert_eq!((img.width, img.height), (2048, 2048));
    // ... while the ordinary document decoder still refuses it.
    assert!(crate::io::png::decode_png(&bytes).is_err());
}

#[test]
fn a_three_slot_sheet_round_trips_its_metadata() {
    let doc = two_layer_doc();
    let mut layout = AtlasLayout::from_document(&doc);
    layout.mode = AtlasMode::Composed;
    layout.columns = 2;
    layout.preset_all = false;
    layout.slots = vec![
        AtlasCellSpec::composed("Both", vec!["layer-1".into(), "layer-top".into()]),
        AtlasCellSpec::single("Top only", "layer-top"),
        AtlasCellSpec::single("Bottom only", "layer-1"),
    ];
    let sheet = build_sheet(&doc, &layout).unwrap();
    assert_eq!((sheet.width, sheet.height), (128, 128));
    assert_eq!(sheet.metadata.grid.rows, 2);
    assert!(sheet.metadata.slots[0].merged);
    assert_eq!(sheet.metadata.slots[2].layer_ids, vec!["layer-1"]);
    assert!(sheet.metadata.sheet_matches_grid());
}
