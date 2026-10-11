use pixel_engine::{io::png::decode_png, pixel::unpack_rgba, recipe::recipe_json_to_document};
use serde_json::Value;

#[test]
fn rust_recipe_renderer_matches_all_legacy_cases() {
    let root = pixel_engine::fixtures::repo_root();
    let manifest: Value =
        serde_json::from_str(include_str!("../../../tools/recipe_cases.json")).unwrap();
    let cases = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .chain(manifest["layer_cases"].as_array().unwrap());
    let mut failures = Vec::new();
    let mut passed = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap_or("?");
        let rel = |key: &str| {
            case[key]
                .as_str()
                .unwrap()
                .strip_prefix("pixeldraw/")
                .unwrap_or(case[key].as_str().unwrap())
        };
        let recipe: Value = match std::fs::read(root.join("asset_work/target").join(rel("recipe")))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
        {
            Some(v) => v,
            None => {
                failures.push(format!("{id}: cannot read recipe"));
                continue;
            }
        };
        let layer = case["layer"].as_str();
        let palette = case.get("palette").unwrap_or(&Value::Null);
        let doc = match recipe_json_to_document(&recipe, layer, None, palette) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{id}: import {e}"));
                continue;
            }
        };
        let expected = match std::fs::read(root.join("asset_work/target").join(rel("legacy")))
            .ok()
            .and_then(|b| decode_png(&b).ok())
        {
            Some(v) => v,
            None => {
                failures.push(format!("{id}: cannot decode legacy PNG"));
                continue;
            }
        };
        let vp = doc
            .canvas
            .viewport
            .unwrap_or(pixel_engine::model::Viewport {
                x: 0,
                y: 0,
                w: expected.width,
                h: expected.height,
            });
        if vp.w != expected.width || vp.h != expected.height {
            failures.push(format!(
                "{id}: output size {}x{} != {}x{}",
                vp.w, vp.h, expected.width, expected.height
            ));
            continue;
        }
        let Some(output) = doc.layers.first() else {
            failures.push(format!("{id}: no rendered layer"));
            continue;
        };
        let (mut matches, total) = (0usize, expected.width as usize * expected.height as usize);
        let mut samples = Vec::new();
        for y in 0..expected.height {
            for x in 0..expected.width {
                let actual = unpack_rgba(output.store.get_pixel(x + vp.x, y + vp.y));
                let o = ((y * expected.width + x) * 4) as usize;
                let want = &expected.rgba[o..o + 4];
                if actual == want || (actual[3] == 0 && want[3] == 0) {
                    matches += 1
                } else if samples.len() < 5 {
                    samples.push((x, y, actual, want.to_vec()));
                }
            }
        }
        let rate = matches as f64 / total as f64;
        if rate < 0.99 {
            failures.push(format!(
                "{id}: parity {rate:.6} ({matches}/{total}); samples {samples:?}"
            ));
        } else {
            passed += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "Rust recipe parity {passed}/39 passed:\n{}",
        failures.join("\n")
    );
    assert_eq!(passed, 39, "shared recipe manifest must contain 39 cases");
}
