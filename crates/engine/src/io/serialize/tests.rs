use super::*;
use crate::chunkstore::PixelWriter;
use crate::model::{NewDocumentOptions, new_document};
#[test]
fn raster_document_round_trips_pixels_and_canvas_metadata() {
    let mut doc = new_document(
        NewDocumentOptions::default()
            .size(64, 64)
            .id("doc-test")
            .name("test"),
    )
    .unwrap();
    {
        let layer = doc.active_layer_mut().unwrap();
        let mut w = PixelWriter::new(&mut layer.store, &layer.id);
        w.set(2, 3, 0x804020ff);
        w.finish();
    }
    let json = document_to_json(&doc).unwrap();
    assert_eq!(json["format"], "draw_tool.document");
    assert_eq!(json["layers"][0]["raster"]["chunks"][0]["cx"], 0);
    let (copy, warnings) = json_to_document(&json).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(copy.id, doc.id);
    assert_eq!(copy.canvas.width_px, 64);
    assert_eq!(
        copy.layers[0].store.get_pixel(2, 3),
        doc.layers[0].store.get_pixel(2, 3)
    );
}
#[test]
fn layer_file_export_import_is_validated_and_keeps_pixels() {
    let mut doc = new_document(NewDocumentOptions::default().size(64, 64).id("doc-test")).unwrap();
    {
        let layer = doc.active_layer_mut().unwrap();
        let mut w = PixelWriter::new(&mut layer.store, &layer.id);
        w.set(4, 5, 0xff336699);
        w.finish();
    }
    let data = layer_to_json(&doc, &doc.active_layer_id).unwrap();
    let (layer, dropped, warnings) = import_layer_json(&mut doc, &data).unwrap();
    assert_eq!(dropped, 0);
    assert!(warnings.is_empty());
    assert_eq!(
        layer.store.get_pixel(4, 5),
        doc.layers[0].store.get_pixel(4, 5)
    );
    assert_ne!(layer.id, doc.layers[0].id);
}
#[test]
fn empty_chunks_are_not_serialized() {
    let doc = new_document(NewDocumentOptions::default().size(32, 32)).unwrap();
    assert_eq!(
        document_to_json(&doc).unwrap()["layers"][0]["raster"]["chunks"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}
#[test]
fn bundled_sample_document_loads_and_reemits_as_valid_schema() {
    let source = include_str!("../../../../../schema/sample_document.json");
    let mut value: serde_json::Value = serde_json::from_str(source).unwrap();
    value["layers"]
        .as_array_mut()
        .unwrap()
        .retain(|layer| layer["type"] == "raster");
    value["active_layer_id"] = value["layers"][0]["layer_id"].clone();
    let (doc, warnings) = json_to_document(&value).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(doc.layers.len(), 2);
    assert_eq!(doc.canvas.width_px, 160);
    let saved = document_to_json(&doc).unwrap();
    assert!(crate::io::validate::validate_document(&saved).ok);
}
