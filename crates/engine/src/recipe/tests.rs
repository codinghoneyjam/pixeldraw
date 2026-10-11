use super::render_commands;
use crate::chunkstore::{ChunkStore, PixelWriter};
use serde_json::json;
#[test]
fn basic_recipe_shapes_render_with_palette_and_color_order() {
    let mut store = ChunkStore::new(32, 32).unwrap();
    {
        let mut writer = PixelWriter::new(&mut store, "test");
        let commands = vec![
            json!({"cmd":"rect","box":[[2,3],[5,6]],"fill":"$body_color"}),
            json!({"cmd":"pixel","x":1,"y":1,"color":"#00ff00"}),
        ];
        render_commands(
            &mut writer,
            &commands,
            &json!({"body_color":"#ff0000"}),
            32,
            32,
        )
        .unwrap();
        writer.finish();
    }
    assert_eq!(store.get_pixel(2, 3), 0xff0000ff);
    assert_eq!(store.get_pixel(1, 1), 0xff00ff00);
    assert_eq!(store.get_pixel(6, 6), 0);
}
#[test]
fn basic_recipe_alpha_overwrites_opacity() {
    let mut store = ChunkStore::new(32, 32).unwrap();
    {
        let mut w = PixelWriter::new(&mut store, "t");
        render_commands(
            &mut w,
            &[json!({"cmd":"alpha","box":[[0,0],[0,0]],"color":"#ff0000","alpha":90})],
            &json!({}),
            32,
            32,
        )
        .unwrap();
        w.finish();
    }
    assert_eq!(store.get_pixel(0, 0), 0x5a0000ff);
}
#[test]
fn sword_albedo_recipe_matches_legacy_png() {
    let recipe_path =
        crate::fixtures::repo_path("asset_work/target/assetdb/entity/weapon/weapon_sword.json");
    let legacy_path =
        crate::fixtures::repo_path("asset_work/target/assetdb/entity/weapon/sword_albedo.png");
    let recipe: serde_json::Value =
        serde_json::from_slice(&std::fs::read(recipe_path).unwrap()).unwrap();
    let mut store = ChunkStore::new(128, 128).unwrap();
    {
        let mut writer = PixelWriter::new(&mut store, "test");
        render_commands(
            &mut writer,
            recipe["albedo_layers"].as_array().unwrap(),
            &recipe["palette"],
            128,
            128,
        )
        .unwrap();
        writer.finish();
    }
    let bytes = std::fs::read(legacy_path).unwrap();
    let expected = crate::io::png::decode_png(&bytes).unwrap();
    let mut matches = 0usize;
    for y in 0..128 {
        for x in 0..128 {
            let actual = crate::pixel::unpack_rgba(store.get_pixel(x, y));
            let o = (y as usize * 128 + x as usize) * 4;
            if actual == expected.rgba[o..o + 4] {
                matches += 1;
            }
        }
    }
    assert!(
        matches as f64 / (128.0 * 128.0) >= 0.99,
        "match rate {:.6}",
        matches as f64 / (128.0 * 128.0)
    );
}

#[test]
fn recipe_document_import_renders_weapon_and_radial_gradient_cases() {
    for (recipe_rel, key, legacy_rel, width, height) in [(
        "asset_work/target/assetdb/entity/weapon/weapon_sword.json",
        Some("albedo"),
        "asset_work/target/assetdb/entity/weapon/sword_albedo.png",
        128,
        128,
    )] {
        let recipe_path = crate::fixtures::repo_path(recipe_rel);
        let recipe: serde_json::Value =
            serde_json::from_slice(&std::fs::read(recipe_path).unwrap()).unwrap();
        let doc =
            crate::recipe::recipe_json_to_document(&recipe, key, None, &serde_json::Value::Null)
                .unwrap();
        assert_eq!(doc.canvas.width_px, width);
        let expected = crate::io::png::decode_png(
            &std::fs::read(crate::fixtures::repo_path(legacy_rel)).unwrap(),
        )
        .unwrap();
        let mut matches = 0usize;
        for y in 0..height {
            for x in 0..width {
                let actual = crate::pixel::unpack_rgba(doc.layers[0].store.get_pixel(x, y));
                let o = (y as usize * width as usize + x as usize) * 4;
                if actual == expected.rgba[o..o + 4] {
                    matches += 1;
                }
            }
        }
        let fraction = matches as f64 / (width as f64 * height as f64);
        assert!(
            fraction >= 0.99,
            "{recipe_rel} match fraction {fraction:.6}"
        );
    }
    let gradient = serde_json::json!({"canvas":[32,32],"layers":[{"id":"g","commands":[{"cmd":"radial_gradient","center":[16,16],"r0":0,"r1":12,"stops":[[0,200,200,200,255],[0.5,200,200,200,255],[1,0,0,0,0]]}]}]});
    let doc =
        crate::recipe::recipe_json_to_document(&gradient, None, None, &serde_json::Value::Null)
            .unwrap();
    assert_eq!(
        crate::pixel::unpack_rgba(doc.layers[0].store.get_pixel(16, 16)),
        [200, 200, 200, 255]
    );
}

#[test]
fn baked_glyph_text_recipe_matches_title_logo() {
    let recipe: serde_json::Value = serde_json::from_slice(
        &std::fs::read(crate::fixtures::repo_path(
            "asset_work/target/tests/fixtures/title_logo_text_our_recipe.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let doc = crate::recipe::recipe_json_to_document(&recipe, None, None, &serde_json::Value::Null)
        .unwrap();
    let expected = crate::io::png::decode_png(
        &std::fs::read(crate::fixtures::repo_path(
            "asset_work/target/assetdb/ui/title/title_logo_text_our.png",
        ))
        .unwrap(),
    )
    .unwrap();
    let (w, h) = (expected.width as usize, expected.height as usize);
    let mut matches = 0;
    for y in 0..h {
        for x in 0..w {
            let actual =
                crate::pixel::unpack_rgba(doc.layers[0].store.get_pixel(x as u32, y as u32));
            let o = (y * w + x) * 4;
            if actual == expected.rgba[o..o + 4] {
                matches += 1
            }
        }
    }
    let rate = matches as f64 / (w * h) as f64;
    assert!(rate >= 0.99, "title logo text match rate {rate:.6}");
}
