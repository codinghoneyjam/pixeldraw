//! Atlas round-trip and grid-import regression tests.

use pixel_engine::atlas::{ATLAS_META_VERSION, AtlasLayout, AtlasMode, GridSliceOptions};
use pixel_engine::io::png::{PngImage, decode_png_large};

use super::*;
use crate::file_ops::{new_document_with_bg, open_png_bytes};

fn paint(state: &mut AppState, layer: &str, x: u32, y: u32, rgba: [u8; 4]) {
    state
        .session
        .stroke_begin(
            pixel_engine::model::edit_session::EditOptions::on_layer(layer).labeled("test"),
        )
        .unwrap();
    state
        .session
        .stroke_with(|w| {
            w.set(
                x,
                y,
                pixel_engine::pixel::pack_rgba_bytes(rgba[0], rgba[1], rgba[2], rgba[3]),
            )
        })
        .unwrap();
    state.session.stroke_commit(Some("test")).unwrap();
}

#[test]
fn grid_atlas_import_accepts_all_sixty_four_document_layers() {
    let image = PngImage {
        width: 256,
        height: 256,
        rgba: vec![0; 256 * 256 * 4],
        metadata: Vec::new(),
    };
    let mut state = AppState::new();
    import_png_grid(
        &mut state,
        &image,
        GridSliceOptions {
            cell_w: 32,
            cell_h: 32,
            columns: 8,
            rows: 8,
            offset_x: 0,
            offset_y: 0,
            spacing_x: 0,
            spacing_y: 0,
            keep_empty: true,
        },
    )
    .unwrap();
    let doc = state.session.require_doc().unwrap();
    assert_eq!(doc.layers.len(), 64);
    assert_eq!(doc.atlas.as_ref().unwrap().cell_count(doc), 64);
}

#[test]
fn atlas_round_trip_returns_to_cell_size_and_preserves_layers() {
    let mut state = AppState::new();
    new_document_with_bg(&mut state, 64, 64, "transparent").unwrap();
    let bottom = state.session.require_doc().unwrap().active_layer_id.clone();
    paint(&mut state, &bottom, 1, 1, [255, 0, 0, 255]);
    let top = state.session.add_layer(Some("Top")).unwrap();
    paint(&mut state, &top, 20, 20, [0, 0, 255, 255]);

    let mut layout = AtlasLayout::from_document(state.session.require_doc().unwrap());
    layout.columns = 2;
    let bytes = export_atlas_bytes(&state, &layout).unwrap();
    let img = decode_png_large(&bytes).unwrap();
    assert_eq!((img.width, img.height), (128, 64));
    let raw = img.metadata_chunk("dtAt").expect("atlas metadata");
    let meta = AtlasMetadata::from_json_bytes(raw).unwrap();
    assert_eq!(meta.version, ATLAS_META_VERSION);
    assert_eq!(meta.mode, AtlasMode::Layers);
    assert_eq!(meta.grid.cols, 2);
    assert_eq!(meta.grid.rows, 1);
    assert_eq!(meta.slots.len(), 2);
    assert_eq!(meta.slots[0].name, "Top");

    open_png_bytes(&mut state, &bytes).unwrap();
    let doc = state.session.require_doc().unwrap();
    assert_eq!(
        (doc.canvas.width_px, doc.canvas.height_px),
        (64, 64),
        "the reopened document is cell-sized, not sheet-sized"
    );
    assert_eq!(
        doc.layers
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Layer 1", "Top"],
        "reopened stack keeps the visual order"
    );
    assert_eq!(
        doc.layers[1].store.get_pixel(20, 20),
        pixel_engine::pixel::pack_rgba(0.0, 0.0, 255.0, 255.0)
    );
    assert_eq!(
        doc.layers[0].store.get_pixel(1, 1),
        pixel_engine::pixel::pack_rgba(255.0, 0.0, 0.0, 255.0)
    );
    assert!(!state.session.history().can_undo(), "reopen starts clean");
}

#[test]
fn atlas_round_trip_restores_margin_padding_and_exact_cell_pixels() {
    let mut state = AppState::new();
    new_document_with_bg(&mut state, 64, 64, "transparent").unwrap();
    let bottom = state.session.require_doc().unwrap().active_layer_id.clone();
    paint(&mut state, &bottom, 1, 1, [255, 0, 0, 255]);
    let top = state.session.add_layer(Some("Top")).unwrap();
    paint(&mut state, &top, 20, 20, [0, 0, 255, 255]);

    let mut layout = AtlasLayout::from_document(state.session.require_doc().unwrap());
    layout.columns = 2;
    layout.margin = 4;
    layout.padding = 8;
    let bytes = export_atlas_bytes(&state, &layout).unwrap();
    let img = decode_png_large(&bytes).unwrap();
    assert_eq!((img.width, img.height), (144, 72));

    open_png_bytes(&mut state, &bytes).unwrap();
    let doc = state.session.require_doc().unwrap();
    let restored = doc.atlas.as_ref().expect("restored atlas layout");
    assert_eq!((restored.columns, restored.rows.unwrap()), (2, 1));
    assert_eq!((restored.margin, restored.padding), (4, 8));
    assert_eq!(doc.layers[0].name, "Layer 1");
    assert_eq!(doc.layers[1].name, "Top");
    assert_eq!(
        doc.layers[1].store.get_pixel(20, 20),
        pixel_engine::pixel::pack_rgba(0.0, 0.0, 255.0, 255.0)
    );
    assert_eq!(
        doc.layers[0].store.get_pixel(1, 1),
        pixel_engine::pixel::pack_rgba(255.0, 0.0, 0.0, 255.0)
    );
    assert!(!state.session.history().can_undo());

    let reexport = export_atlas_bytes(&state, restored).unwrap();
    let reexported = decode_png_large(&reexport).unwrap();
    assert_eq!(
        (reexported.width, reexported.height),
        (img.width, img.height)
    );
    assert_eq!(
        reexported.rgba, img.rgba,
        "atlas cell pixels must round-trip exactly"
    );
}

#[test]
fn composed_atlas_import_bakes_one_layer_per_slot() {
    let mut state = AppState::new();
    new_document_with_bg(&mut state, 64, 64, "transparent").unwrap();
    let bottom = state.session.require_doc().unwrap().active_layer_id.clone();
    paint(&mut state, &bottom, 1, 1, [255, 0, 0, 255]);
    let top = state.session.add_layer(Some("Top")).unwrap();
    paint(&mut state, &top, 20, 20, [0, 0, 255, 255]);

    let mut layout = AtlasLayout::from_document(state.session.require_doc().unwrap());
    layout.mode = AtlasMode::Composed;
    layout.columns = 1;
    let bytes = export_atlas_bytes(&state, &layout).unwrap();
    open_png_bytes(&mut state, &bytes).unwrap();
    let doc = state.session.require_doc().unwrap();
    assert_eq!((doc.canvas.width_px, doc.canvas.height_px), (64, 64));
    assert_eq!(
        doc.layers
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        vec!["All Layers"]
    );
    assert_eq!(
        doc.layers[0].store.get_pixel(1, 1),
        pixel_engine::pixel::pack_rgba(255.0, 0.0, 0.0, 255.0)
    );
    assert_eq!(
        doc.layers[0].store.get_pixel(20, 20),
        pixel_engine::pixel::pack_rgba(0.0, 0.0, 255.0, 255.0)
    );
}
