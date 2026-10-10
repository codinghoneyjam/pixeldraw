//! Shared fixture harness — the Rust side of the parity gate.
//!
//! The JS tree's golden vectors live in `tests/fixtures/*.json` and were baked by
//! `tools/gen_fixtures.py` against a PIL oracle. They are loaded through this
//! module by **both** the in-crate unit tests and the integration tests, so there
//! is exactly one reader and a mismatch in either direction fails the build.
//!
//! Group names in the JSON map 1:1 to test groups: `brush`, `ellipse`, `rrect`,
//! `outline`, `bresenham`, `stroke`, `wide_line`, `polygon`, `arc`, ...
//!
//! `serde_json` is a normal dependency rather than a dev one precisely so this
//! can live in the library; the engine itself starts using it for the schema
//! serializer in Phase 1.13.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Walk up from the crate to the repo root, so this works whether cargo was
/// invoked from the workspace root or from `crates/engine`.
pub fn repo_root() -> &'static Path {
    ROOT.get_or_init(|| {
        let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        loop {
            if dir.join("tests/fixtures").is_dir() && dir.join("src/core").is_dir() {
                return dir;
            }
            if !dir.pop() {
                panic!("could not locate repo root from CARGO_MANIFEST_DIR");
            }
        }
    })
}

/// Path of a repo-relative file.
pub fn repo_path(rel: &str) -> PathBuf {
    repo_root().join(rel)
}

/// Parsed `tests/fixtures/raster_golden.json`.
pub fn golden() -> &'static serde_json::Value {
    static GOLDEN: OnceLock<serde_json::Value> = OnceLock::new();
    GOLDEN.get_or_init(|| {
        let text = std::fs::read_to_string(repo_path("tests/fixtures/raster_golden.json"))
            .expect("tests/fixtures/raster_golden.json");
        serde_json::from_str(&text).expect("raster_golden.json parses")
    })
}

/// One group of the golden file, e.g. `"brush"`.
pub fn group(name: &str) -> &'static Vec<serde_json::Value> {
    golden()
        .get(name)
        .unwrap_or_else(|| panic!("golden fixture has no group `{name}`"))
        .as_array()
        .unwrap_or_else(|| panic!("golden group `{name}` is not an array"))
}

/// Number of cases in a group — pins the fixture so silent edits fail.
pub fn case_count(name: &str) -> usize {
    group(name).len()
}

/// `rows: ["##..", "#..#"]` → `vec![vec![true, true, false, false], ...]`.
pub fn rows_to_mask(case: &serde_json::Value) -> Vec<Vec<bool>> {
    case["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("case has no rows: {case}"))
        .iter()
        .map(|r| {
            r.as_str()
                .expect("row is a string")
                .chars()
                .map(|c| match c {
                    '#' => true,
                    '.' => false,
                    other => panic!("row char {other:?} is neither # nor ."),
                })
                .collect()
        })
        .collect()
}

/// Render a mask back to the fixture's `#`/`.` text.
pub fn mask_to_rows(mask: &[Vec<bool>]) -> Vec<String> {
    mask.iter()
        .map(|row| {
            row.iter()
                .map(|&b| if b { '#' } else { '.' })
                .collect::<String>()
        })
        .collect()
}

/// `[r, g, b, a]` from a JSON array.
pub fn rgba(value: &serde_json::Value) -> [u8; 4] {
    let a = value.as_array().expect("rgba array");
    [
        a[0].as_u64().expect("r") as u8,
        a[1].as_u64().expect("g") as u8,
        a[2].as_u64().expect("b") as u8,
        a[3].as_u64().expect("a") as u8,
    ]
}

/// A packed u32 from a fixture hex string like `"0A141EFF"`.
pub fn packed_hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap_or_else(|e| panic!("bad hex {s}: {e}"))
}

/// u32 field helper.
pub fn u32_field(case: &serde_json::Value, key: &str) -> u32 {
    case[key]
        .as_u64()
        .unwrap_or_else(|| panic!("missing u32 `{key}` in {case}")) as u32
}

/// usize field helper.
pub fn usize_field(case: &serde_json::Value, key: &str) -> usize {
    case[key]
        .as_u64()
        .unwrap_or_else(|| panic!("missing usize `{key}` in {case}")) as usize
}

/// i64 field helper (fixtures carry negative coordinates).
pub fn i64_field(case: &serde_json::Value, key: &str) -> i64 {
    case[key]
        .as_i64()
        .unwrap_or_else(|| panic!("missing i64 `{key}` in {case}"))
}

/// f64 field helper.
pub fn f64_field(case: &serde_json::Value, key: &str) -> f64 {
    case[key]
        .as_f64()
        .unwrap_or_else(|| panic!("missing f64 `{key}` in {case}"))
}

/// String field helper.
pub fn str_field<'a>(case: &'a serde_json::Value, key: &str) -> &'a str {
    case[key]
        .as_str()
        .unwrap_or_else(|| panic!("missing string `{key}` in {case}"))
}

/// Assert a mask equals the fixture, printing a row diff on failure.
pub fn assert_mask(case: &serde_json::Value, got: &[Vec<bool>]) {
    let want = rows_to_mask(case);
    if want == got {
        return;
    }
    let name = case["name"].as_str().unwrap_or("<unnamed>");
    let want_rows = mask_to_rows(&want);
    let got_rows = mask_to_rows(got);
    let mut msg = format!("mask mismatch: {name}\n  want | got\n");
    for i in 0..want_rows.len().max(got_rows.len()) {
        let w = want_rows.get(i).cloned().unwrap_or_default();
        let g = got_rows.get(i).cloned().unwrap_or_default();
        let mark = if w == g { " " } else { "<" };
        msg.push_str(&format!("{mark} {w} | {g}\n"));
    }
    panic!("{msg}");
}
