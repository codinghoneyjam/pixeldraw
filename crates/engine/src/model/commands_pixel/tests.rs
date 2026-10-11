use super::merge_down::MergeDownCommand;
use super::resize::ResizeCanvasCommand;
use crate::blend::over;
use crate::constants::CHUNK_LEN;
use crate::model::command::{Command, PixelEffect};
use crate::model::document::{Document, NewDocumentOptions, new_document};
use crate::model::layer::Layer;
use crate::pixel::unpack_rgba;

const RED: u32 = 0xff0000ff;
const WHITE: u32 = 0xffffffff;
/// Black is `0xff000000`, not the red I first pasted in — that swapped the
/// merge from "white over black" to "white over red" and the golden value
/// `[128,128,128]` caught it.
const BLACK: u32 = 0xff000000;

fn doc() -> Document {
    new_document(NewDocumentOptions::default().size(32, 32)).expect("valid")
}

/// The `paintPixel` helper of `tests/helpers/model.js`, against a raw store.
fn dot(store: &mut crate::chunkstore::ChunkStore, x: u32, y: u32, packed: u32) {
    store.set_pixel(x, y, packed);
}

// <META - ROLE : parity with tests/model_history.test.mjs B5.6 merge cases | L126-199>
#[test]
fn merge_down_matches_the_composite_and_round_trips() {
    let mut d = doc();
    let lower_id = d.active_layer_id.clone();
    dot(&mut d.layers[0].store, 0, 0, BLACK);
    let mut upper = Layer::create(d.ids.next(), "U", 32, 32).expect("ok");
    let upper_id = upper.id.clone();
    dot(&mut upper.store, 0, 0, WHITE);
    d.insert(upper, 1).expect("ok");
    d.layers[1].opacity = 1.0;
    d.layers[0].opacity = 1.0;

    let mut cmd = MergeDownCommand::new(upper_id);
    Command::apply(&mut cmd, &mut d).expect("merge");
    // The JS expectation: `over(over(transparent, black, 1), white, 1)`.
    let got = unpack_rgba(d.get_layer(&lower_id).unwrap().store.get_pixel(0, 0));
    assert_eq!(
        got,
        over(
            over([0, 0, 0, 0], [0, 0, 0, 255], 1.0),
            [255, 255, 255, 255],
            1.0
        )
    );
    assert_eq!(d.layers.len(), 1);

    Command::revert(&mut cmd, &mut d).expect("undo");
    assert_eq!(d.layers.len(), 2);
    assert_eq!(
        unpack_rgba(d.get_layer(&lower_id).unwrap().store.get_pixel(0, 0)),
        [0, 0, 0, 255]
    );
    assert_eq!(
        unpack_rgba(d.get_layer(&cmd.upper_id).unwrap().store.get_pixel(0, 0)),
        [255, 255, 255, 255]
    );
    Command::apply(&mut cmd, &mut d).expect("redo");
    assert_eq!(d.layers.len(), 1);
}

#[test]
fn merge_down_bakes_opacity_and_sets_it_to_one() {
    let mut d = doc();
    let lower_id = d.active_layer_id.clone();
    dot(&mut d.layers[0].store, 3, 3, BLACK);
    let mut upper = Layer::create(d.ids.next(), "upper", 32, 32).expect("ok");
    let upper_id = upper.id.clone();
    dot(&mut upper.store, 3, 3, WHITE);
    upper.opacity = 0.5;
    d.insert(upper, 1).expect("ok");
    let mut cmd = MergeDownCommand::new(upper_id);
    Command::apply(&mut cmd, &mut d).expect("merge");
    // Golden: white@0.5 over black is [128,128,128,255].
    assert_eq!(
        unpack_rgba(d.get_layer(&lower_id).unwrap().store.get_pixel(3, 3)),
        [128, 128, 128, 255]
    );
    assert_eq!(d.get_layer(&lower_id).unwrap().opacity, 1.0);
    assert_eq!(d.active_layer_id, lower_id);
}

#[test]
fn merge_down_rejects_locked_hidden_and_the_bottom_layer() {
    let mut d = doc();
    let bottom = d.active_layer_id.clone();
    let top = Layer::create(d.ids.next(), "top", 32, 32).expect("ok");
    let top_id = top.id.clone();
    d.insert(top, 1).expect("ok");

    let mut m = MergeDownCommand::new(bottom.clone());
    assert_eq!(
        Command::apply(&mut m, &mut d).unwrap_err().code,
        crate::errors::CODE_INVALID_STATE
    );
    d.get_layer_mut(&top_id).unwrap().locked = true;
    let mut m = MergeDownCommand::new(top_id.clone());
    assert_eq!(
        Command::apply(&mut m, &mut d).unwrap_err().code,
        crate::errors::CODE_LAYER_LOCKED
    );
    d.get_layer_mut(&top_id).unwrap().locked = false;
    d.get_layer_mut(&bottom).unwrap().visible = false;
    let mut m = MergeDownCommand::new(top_id.clone());
    assert_eq!(
        Command::apply(&mut m, &mut d).unwrap_err().code,
        crate::errors::CODE_LAYER_HIDDEN
    );
    // Nothing moved.
    assert_eq!(d.layers.len(), 2);
}

#[test]
fn merge_down_byte_size_counts_both_snapshots() {
    let mut d = doc();
    let lower_id = d.active_layer_id.clone();
    // Both layers hold a chunk, so the snapshot is one chunk each side.
    dot(&mut d.layers[0].store, 0, 0, BLACK);
    let mut upper = Layer::create(d.ids.next(), "U", 32, 32).expect("ok");
    dot(&mut upper.store, 0, 0, WHITE);
    d.insert(upper, 1).expect("ok");
    let upper_id = d.layers[1].id.clone();
    let mut cmd = MergeDownCommand::new(upper_id);
    assert_eq!(Command::byte_size(&cmd), 0);
    Command::apply(&mut cmd, &mut d).expect("merge");
    assert_eq!(Command::byte_size(&cmd), CHUNK_LEN * 2);
    // Undo restores the lower's chunk count, so the size does not drift.
    Command::revert(&mut cmd, &mut d).expect("undo");
    assert_eq!(d.get_layer(&lower_id).unwrap().store.chunk_count(), 1);
}

#[test]
fn resize_canvas_round_trip_and_redo_keeps_the_original_size() {
    let mut d = new_document(NewDocumentOptions::default().size(64, 64)).expect("valid");
    let id = d.active_layer_id.clone();
    dot(&mut d.get_layer_mut(&id).unwrap().store, 60, 60, RED);
    let mut cmd = ResizeCanvasCommand::new(32, 32).expect("valid");
    assert!(ResizeCanvasCommand::new(33, 32).is_err());
    assert_eq!(cmd.label(), "캔버스 크기 변경");
    assert_eq!(cmd.type_name(), "resize-canvas");
    Command::apply(&mut cmd, &mut d).expect("shrink");
    assert_eq!(d.canvas.width_px, 32);
    assert_eq!(d.get_layer(&id).unwrap().store.get_pixel(60, 60), 0);
    assert_eq!(Command::byte_size(&cmd), CHUNK_LEN);
    Command::revert(&mut cmd, &mut d).expect("undo");
    assert_eq!(d.canvas.width_px, 64);
    assert_eq!(d.get_layer(&id).unwrap().store.get_pixel(60, 60), RED);
    // A do/undo/do cycle keeps the size captured on the first `do`.
    Command::apply(&mut cmd, &mut d).expect("redo");
    Command::revert(&mut cmd, &mut d).expect("undo again");
    assert_eq!(d.canvas.width_px, 64);
}

#[test]
fn resize_canvas_effects_name_every_layer() {
    let mut d = doc();
    let mut extra = Layer::create(d.ids.next(), "S", 32, 32).expect("ok");
    dot(&mut extra.store, 0, 0, RED);
    d.insert(extra, 1).expect("ok");
    let mut cmd = ResizeCanvasCommand::new(64, 64).expect("valid");
    Command::apply(&mut cmd, &mut d).expect("grow");
    let fx = Command::effects(&cmd);
    assert!(fx.layers.is_none());
    assert_eq!(fx.pixels.len(), 2);
    assert!(
        fx.pixels
            .iter()
            .all(|p| matches!(p, PixelEffect::All { .. }))
    );
}
