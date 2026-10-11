use super::layer_cmd::{AddLayerCommand, MoveLayerCommand};
use super::paint::PaintCommand;
use super::prop::SetLayerPropCommand;
use crate::constants::CHUNK_LEN;
use crate::model::command::{Command, LayerChangeReason, PixelEffect};
use crate::model::document::{Document, NewDocumentOptions, new_document};
use crate::model::layer::{Layer, LayerField, LayerPropValue};
use crate::pixel::pack_rgba;

const RED: u32 = 0xff0000ff;

fn doc() -> Document {
    new_document(NewDocumentOptions::default().size(32, 32)).expect("valid")
}

// <META - ROLE : parity with helpers/model.js paintPixel + PaintCommand cases | L27-34>
#[test]
fn paint_do_undo_do_restores_the_bytes() {
    let mut d = doc();
    let id = d.active_layer_id.clone();
    let mut writer =
        crate::chunkstore::PixelWriter::new(&mut d.active_layer_mut().unwrap().store, id.clone());
    writer.set(1, 1, RED);
    let cs = writer.finish().expect("changeset");
    let mut cmd = PaintCommand::with_default_label(cs);
    assert_eq!(cmd.label(), "연필");
    assert_eq!(cmd.type_name(), "paint");
    assert!(cmd.byte_size() >= CHUNK_LEN);

    let pixel = d.get_layer(&id).unwrap().store.get_pixel(1, 1);
    assert_eq!(pixel, RED);
    cmd.revert(&mut d).expect("undo");
    assert_eq!(d.get_layer(&id).unwrap().store.get_pixel(1, 1), 0);
    assert_eq!(d.get_layer(&id).unwrap().store.chunk_count(), 0);
    cmd.apply(&mut d).expect("do");
    assert_eq!(d.get_layer(&id).unwrap().store.get_pixel(1, 1), RED);
}

#[test]
fn add_layer_round_trips() {
    let mut d = doc();
    let first = d.layers[0].id.clone();
    let layer = Layer::create(d.ids.next(), "Second", 32, 32).expect("ok");
    let second = layer.id.clone();
    let mut cmd = AddLayerCommand::with_defaults(layer, 1);
    assert_eq!(cmd.label(), "레이어 추가");
    assert_eq!(cmd.byte_size(), 0);
    cmd.apply(&mut d).expect("ok");
    assert_eq!(d.layers.len(), 2);
    assert_eq!(d.active_layer_id, second);
    cmd.revert(&mut d).expect("ok");
    assert_eq!(d.layers.len(), 1);
    assert_eq!(d.active_layer_id, first);
    cmd.apply(&mut d).expect("ok");
    assert_eq!(d.layers.len(), 2);
}

#[test]
fn duplicate_emits_all_when_the_layer_holds_pixels() {
    let mut d = doc();
    d.layers[0]
        .store
        .set_pixel(0, 0, pack_rgba(1.0, 2.0, 3.0, 255.0));
    let clone = d.layers[0]
        .clone_with(d.ids.next(), "Layer 1 copy")
        .expect("ok");
    let cmd = AddLayerCommand::with_defaults(clone, 1);
    let fx = Command::effects(&cmd);
    assert_eq!(
        fx.pixels,
        vec![PixelEffect::All {
            layer_id: cmd.layer.id.clone()
        }]
    );
    assert!(
        fx.layers
            .is_some_and(|l| l.reason == LayerChangeReason::Add)
    );
}

#[test]
fn set_layer_prop_merges_only_opacity_on_the_same_layer() {
    let d = doc();
    let id = d.active_layer_id.clone();
    let mut a = SetLayerPropCommand::with_label(
        id.clone(),
        LayerField::Opacity,
        LayerPropValue::Opacity(1.0),
        LayerPropValue::Opacity(0.5),
        "o",
    );
    let mut b = SetLayerPropCommand::with_label(
        id.clone(),
        LayerField::Opacity,
        LayerPropValue::Opacity(0.5),
        LayerPropValue::Opacity(0.3),
        "o",
    );
    assert!(b.merge_with(&mut a));
    assert_eq!(a.after, LayerPropValue::Opacity(0.3));
    // A different field never merges.
    let mut named = SetLayerPropCommand::with_label(
        id.clone(),
        LayerField::Name,
        LayerPropValue::Name("a".into()),
        LayerPropValue::Name("b".into()),
        "o",
    );
    let mut opacity_new = SetLayerPropCommand::with_label(
        "layer-9999".to_string(),
        LayerField::Opacity,
        LayerPropValue::Opacity(1.0),
        LayerPropValue::Opacity(0.1),
        "o",
    );
    assert!(!opacity_new.merge_with(&mut a));
    assert!(!named.merge_with(&mut a));
    // A non-prop command never merges.
    let other =
        AddLayerCommand::with_defaults(Layer::create("layer-0009", "x", 32, 32).expect("ok"), 1);
    let mut boxed: Box<dyn Command> = Box::new(other);
    assert!(!opacity_new.merge_with(boxed.as_mut()));
}

#[test]
fn move_command_swaps_by_index() {
    let mut d = doc();
    let second = Layer::create(d.ids.next(), "S", 32, 32).expect("ok");
    let sid = second.id.clone();
    d.insert(second, 1).expect("ok");
    let mut cmd = MoveLayerCommand::new(sid.clone(), 1, 0);
    cmd.apply(&mut d).expect("ok");
    assert_eq!(d.layers[0].id, sid);
    cmd.revert(&mut d).expect("ok");
    assert_eq!(d.layers[1].id, sid);
    assert_eq!(Command::byte_size(&cmd), 0);
}
