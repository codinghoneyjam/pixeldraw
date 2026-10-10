//! Phase 0.4 gate: the fixture is readable from Rust and its shape is pinned.
//!
//! This is the harness smoke test. The real per-group parity tests land with
//! Phase 1.4+ (`fixture_golden.rs`), but this catches a moved/renamed/reshaped
//! fixture immediately — before an engine port silently grades itself against
//! nothing.

use pixel_engine::fixtures::{case_count, golden, group, repo_path};

// <META - ROLE : group sizes as baked on 2026-10-10; bump only with the fixture | L8-24>
#[test]
fn golden_groups_are_pinned() {
    let expected: &[(&str, usize)] = &[
        ("brush", 14),
        ("ellipse", 15),
        ("rrect", 8),
        ("outline", 10),
        ("bresenham", 7),
        ("stroke", 7),
        ("angle_snap", 8),
        ("resize", 15),
        ("drag_bbox", 8),
        ("unit_snap", 6),
        ("composite", 8),
        ("fill", 3),
        ("polygon", 6),
        ("polygon_outline", 6),
        ("fillet", 5),
        ("gradient", 2),
        ("arc", 2),
        ("chord", 2),
        ("wide_line", 54),
        ("paste", 5),
        ("hashes", 6),
    ];
    for (name, want) in expected {
        assert_eq!(
            case_count(name),
            *want,
            "golden group `{name}` changed size — the parity target moved"
        );
    }
    // Sum check so a brand-new group cannot hide.
    let total: usize = expected.iter().map(|(_, n)| n).sum();
    assert_eq!(group("brush").len() + total - case_count("brush"), total);
}

#[test]
fn golden_declares_itself_normative() {
    let doc = golden()["_doc"].as_str().expect("_doc string");
    assert!(
        doc.contains("Normative test vectors"),
        "fixture no longer declares itself normative: {doc}"
    );
}

#[test]
fn every_case_has_rows_or_data() {
    // Groups whose cases carry `rows`. The data-shaped groups (composite/fill/
    // paste/hashes/bresenham) are covered by their own assertions in Phase 1.
    for name in [
        "brush",
        "ellipse",
        "rrect",
        "outline",
        "polygon",
        "wide_line",
    ] {
        for case in group(name) {
            assert!(case.get("rows").is_some(), "`{name}` case without rows");
        }
    }
}

// <META - ROLE : the JS oracle manifest must still match, or parity means nothing | L48-58>
#[test]
fn oracle_manifest_is_present_and_current() {
    let path = repo_path("tools/oracle-manifest.json");
    let text = std::fs::read_to_string(&path)
        .expect("tools/oracle-manifest.json — run `node tools/oracle_manifest.mjs`");
    let manifest: serde_json::Value = serde_json::from_str(&text).expect("manifest parses");
    let files = manifest["files"].as_object().expect("files object");
    assert!(
        files.len() > 150,
        "manifest looks truncated: {} files",
        files.len()
    );
    // Spot-check that the frozen sources the engine is replacing are all listed.
    for must in [
        "src/core/blend.js",
        "src/core/chunkstore.js",
        "src/core/raster/polygon.js",
        "reference/raster_ref.mjs",
    ] {
        assert!(files.contains_key(must), "manifest is missing {must}");
    }
}
