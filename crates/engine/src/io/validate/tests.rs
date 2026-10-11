use super::{validate_document, validate_layer_file};
use serde_json::json;
#[test]
fn rejects_non_object_and_bad_header() {
    assert_eq!(validate_document(&json!(null)).errors[0].path, "");
    let report = validate_document(&json!({"format":"wrong"}));
    assert!(!report.ok);
    assert_eq!(report.errors[0].code, "SCHEMA");
}
#[test]
fn detects_duplicate_layer_ids_after_structure_pass() {
    let layer = json!({"layer_id":"a","name":"a","type":"raster","visible":true,"locked":false,"opacity":1,"blend":"normal","raster":{"chunk_px":32,"encoding":"png_base64","chunks":[]}});
    let doc = json!({"schema_version":"2.0.0","format":"draw_tool.document","document_id":"d","canvas":{"tile_px":64,"unit_px":32,"width_px":32,"height_px":32},"layers":[layer.clone(),layer],"active_layer_id":"a"});
    let report = validate_document(&doc);
    assert!(report.errors.iter().any(|e| e.code == "DUP_LAYER_ID"));
}
#[test]
fn rejects_invalid_layer_file_shape() {
    let report = validate_layer_file(&json!({"schema_version":"0"}));
    assert!(!report.ok);
    assert!(report.errors.iter().any(|e| e.path == "/layer"));
}
