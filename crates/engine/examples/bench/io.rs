//! Flattening, PNG and document-save benchmark workloads.

use super::super::*;
use pixel_engine::io::serialize::document_to_json;

pub(crate) fn bench_document_to_json() {
    let doc = make_busy_document();
    let mut last: Option<serde_json::Value> = None;
    time_it("documentToJson (save)", PNG_RUNS, || {
        last = Some(document_to_json(&doc).expect("valid document"));
    });
    let value = last.expect("timed at least once");

    // The strings are built once for the size row, then re-built under the
    // timer — the same split the JS bench uses for its `json size` line.
    let compact = serde_json::to_vec(&value).expect("compact encode");
    let pretty = serde_json::to_vec_pretty(&value).expect("pretty encode");
    let encoded_chunks: usize = value["layers"]
        .as_array()
        .map(|layers| {
            layers
                .iter()
                .map(|l| l["raster"]["chunks"].as_array().map_or(0, Vec::len))
                .sum()
        })
        .unwrap_or(0);
    println!(
        "{:<38} | {:>6} chunks encoded ({LAYERS} layers)",
        "  -> source", encoded_chunks
    );
    println!(
        "{:<38} | {:>6} KB compact / {:>6} KB pretty",
        "  -> json size",
        compact.len() / 1024,
        pretty.len() / 1024
    );
    time_it("-> to_vec (compact)", PNG_RUNS, || {
        let bytes = serde_json::to_vec(&value).expect("compact encode");
        assert!(!bytes.is_empty());
    });
    time_it("-> to_vec_pretty", PNG_RUNS, || {
        let bytes = serde_json::to_vec_pretty(&value).expect("pretty encode");
        assert!(!bytes.is_empty());
    });

    // Same spirit as the JS `text.length` guard: a serializer that silently
    // drops layers must not read as a fast one.
    assert!(
        value["layers"]
            .as_array()
            .is_some_and(|layers| layers.len() == LAYERS),
        "all layers must serialize"
    );
}

// ---------------------------------------------------------------------------
// Warm-up + main
