//! The session-level layer operations.
//!
//! Ported 1:1 from `src/features/layers/layer_operations.js`. Every function
//! takes a `&mut Session` (the JS took the session as its first argument) and
//! either runs a command through [`Session::execute`] or, for
//! [`set_active_layer`], mutates the document directly —the one operation with
//! no history entry in the JS tree, because selecting a layer is not undoable.

use crate::constants::is_valid_canvas_size;
use crate::errors::{CODE_CANVAS_SIZE_INVALID, DrawToolError, EngineResult};
use crate::model::command::LayerChangeReason;
use crate::model::commands::{
    AddLayerCommand, MoveLayerCommand, RemoveLayerCommand, SetLayerPropCommand,
};
use crate::model::commands_pixel::{MergeDownCommand, ResizeCanvasCommand};
use crate::model::layer::{
    Layer, LayerField, LayerPropValue, validate_layer_blend, validate_layer_name,
    validate_layer_opacity,
};
use crate::model::session::Session;

/// `addLayer(session, name)` —returns the new layer's id.
pub fn add_layer(session: &mut Session, name: Option<&str>) -> EngineResult<String> {
    let (layer, at) = {
        let doc = session.doc_mut()?;
        let layer_name = name
            .map(str::to_string)
            .unwrap_or_else(|| format!("Layer {}", doc.layers.len() + 1));
        let layer = Layer::create(
            doc.ids.next(),
            layer_name,
            doc.canvas.width_px,
            doc.canvas.height_px,
        )?;
        (layer, doc.layers.len())
    };
    let id = layer.id.clone();
    session.execute(AddLayerCommand::with_defaults(layer, at))?;
    Ok(id)
}

/// `duplicateLayer(session, id)` — the clone lands directly above its source.
pub fn duplicate_layer(session: &mut Session, id: &str) -> EngineResult<String> {
    let (clone, at) = {
        let doc = session.doc_mut()?;
        let (name, at) = {
            let src = doc.get_layer(id)?;
            (
                format!("{} copy", src.name),
                doc.index_of(id)
                    .ok_or_else(|| DrawToolError::layer_not_found(id))?
                    + 1,
            )
        };
        let new_id = doc.ids.next();
        let clone = doc.get_layer(id)?.clone_with(new_id, name)?;
        (clone, at)
    };
    let id = clone.id.clone();
    session.execute(AddLayerCommand::new(clone, at, true, "레이어 복제"))?;
    Ok(id)
}

pub fn remove_layer(session: &mut Session, id: &str) -> EngineResult<()> {
    session.execute(RemoveLayerCommand::new(id))
}

/// `moveLayer(session, id, toIndex)`.
///
/// The **order of the guards is part of the behaviour**: a `toIndex` equal to
/// the current position returns `false` without a history entry *before* the
/// range check runs, so `moveLayer(id, id_index)` is a no-op while
/// `moveLayer(id, -1)` is an `OUT_OF_RANGE` error.
///
/// `to_index` is `i64` because `layerDown` on the bottom layer legitimately
/// passes `-1`.
pub fn move_layer(session: &mut Session, id: &str, to_index: i64) -> EngineResult<bool> {
    let from = {
        let doc = session.require_doc()?;
        doc.index_of(id)
            .ok_or_else(|| DrawToolError::layer_not_found(id))?
    };
    if from as i64 == to_index {
        return Ok(false);
    }
    if to_index < 0 || to_index >= session.require_doc()?.layers.len() as i64 {
        return Err(DrawToolError::out_of_range(format!(
            "move index {to_index} out of range"
        )));
    }
    session.execute(MoveLayerCommand::new(id, from, to_index as usize))?;
    Ok(true)
}

pub fn merge_down(session: &mut Session, id: &str) -> EngineResult<()> {
    session.execute(MergeDownCommand::new(id))
}

/// `insertLayer(session, layer, index)` —`None` puts it after the active layer.
pub fn insert_layer(
    session: &mut Session,
    layer: Layer,
    index: Option<usize>,
) -> EngineResult<String> {
    let at = match index {
        Some(i) => i,
        None => session.require_doc()?.active_index() + 1,
    };
    let id = layer.id.clone();
    session.execute(AddLayerCommand::with_defaults(layer, at))?;
    Ok(id)
}

pub fn resize_canvas(session: &mut Session, w: u32, h: u32) -> EngineResult<bool> {
    {
        let doc = session.require_doc()?;
        if doc.canvas.width_px == w && doc.canvas.height_px == h {
            return Ok(false);
        }
        if !is_valid_canvas_size(w, h) {
            return Err(DrawToolError::new(
                CODE_CANVAS_SIZE_INVALID,
                format!("invalid canvas size {w}x{h}"),
            ));
        }
    }
    session.execute(ResizeCanvasCommand::new(w, h)?)?;
    Ok(true)
}

pub fn rename_layer(session: &mut Session, id: &str, name: &str) -> EngineResult<bool> {
    validate_layer_name(name)?;
    let current = {
        let doc = session.require_doc()?;
        let layer = doc.get_layer(id)?;
        if layer.name == name {
            return Ok(false);
        }
        layer.name.clone()
    };
    session.execute(SetLayerPropCommand::with_label(
        id,
        LayerField::Name,
        LayerPropValue::Name(current),
        LayerPropValue::Name(name.to_string()),
        "레이어 이름 변경",
    ))?;
    Ok(true)
}

pub fn set_layer_visible(session: &mut Session, id: &str, visible: bool) -> EngineResult<bool> {
    prop(session, id, LayerField::Visible, |layer| {
        let next = visible;
        if layer.visible == next {
            return None;
        }
        Some((
            LayerPropValue::Visible(layer.visible),
            LayerPropValue::Visible(next),
            "레이어 표시 변경",
        ))
    })
}

pub fn set_layer_locked(session: &mut Session, id: &str, locked: bool) -> EngineResult<bool> {
    prop(session, id, LayerField::Locked, |layer| {
        let next = locked;
        if layer.locked == next {
            return None;
        }
        Some((
            LayerPropValue::Locked(layer.locked),
            LayerPropValue::Locked(next),
            "레이어 잠금 변경",
        ))
    })
}

pub fn set_layer_blend(session: &mut Session, id: &str, value: &'static str) -> EngineResult<bool> {
    validate_layer_blend(value)?;
    prop(session, id, LayerField::Blend, move |layer| {
        if layer.blend == value {
            return None;
        }
        Some((
            LayerPropValue::Blend(layer.blend),
            LayerPropValue::Blend(value),
            "레이어 블렌드 변경",
        ))
    })
}

/// `setLayerOpacity(session, id, value, { final })`.
///
/// `final` matters only when the value is unchanged: it still calls
/// `breakMerge`, so the next opacity change starts a new history entry. That is
/// the difference between dragging a slider (one entry) and nudging the field
/// (one entry per nudge).
pub fn set_layer_opacity(
    session: &mut Session,
    id: &str,
    value: f64,
    final_: bool,
) -> EngineResult<bool> {
    validate_layer_opacity(value)?;
    let current = {
        let doc = session.require_doc()?;
        let layer = doc.get_layer(id)?;
        if layer.opacity == value {
            if final_ {
                session.history_mut().break_merge();
            }
            return Ok(false);
        }
        layer.opacity
    };
    session.execute(SetLayerPropCommand::with_label(
        id,
        LayerField::Opacity,
        LayerPropValue::Opacity(current),
        LayerPropValue::Opacity(value),
        "레이어 불투명도",
    ))?;
    if final_ {
        session.history_mut().break_merge();
    }
    Ok(true)
}

/// `setActiveLayer(session, id)`.
///
/// The only operation that dispatches `layers-changed` itself rather than going
/// through a command's `effects()`, because it has no `do`/`undo` to record.
pub fn set_active_layer(session: &mut Session, id: &str) -> EngineResult<bool> {
    {
        let doc = session.require_doc()?;
        doc.get_layer(id)?;
        if doc.active_layer_id == id {
            return Ok(false);
        }
    }
    session.doc_mut()?.set_active(id)?;
    session.emit_layers_changed(LayerChangeReason::Active, vec![id.to_string()]);
    Ok(true)
}

/// The shared shape of the four property setters: read the layer, compare,
/// and only then build a `SetLayerPropCommand`.
fn prop(
    session: &mut Session,
    id: &str,
    field: LayerField,
    build: impl FnOnce(&Layer) -> Option<(LayerPropValue, LayerPropValue, &'static str)>,
) -> EngineResult<bool> {
    let (before, after, label) = {
        let layer = session.require_doc()?.get_layer(id)?;
        match build(layer) {
            Some(triple) => triple,
            None => return Ok(false),
        }
    };
    session.execute(SetLayerPropCommand::with_label(
        id, field, before, after, label,
    ))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Session;
    use crate::model::document::NewDocumentOptions;

    fn session() -> Session {
        let mut s = Session::new();
        s.new_document(NewDocumentOptions::default().size(32, 32))
            .expect("new");
        s
    }

    #[test]
    fn add_and_duplicate_round_trip() {
        let mut s = session();
        let first = s.doc().unwrap().active_layer_id.clone();
        let id = add_layer(&mut s, Some("second")).expect("add");
        assert_eq!(s.doc().unwrap().layers.len(), 2);
        assert_eq!(s.doc().unwrap().layers[1].name, "second");
        assert_ne!(id, first);
        let dup = duplicate_layer(&mut s, &id).expect("dup");
        assert_eq!(s.doc().unwrap().layers.len(), 3);
        assert_eq!(s.doc().unwrap().layers[2].name, "second copy");
        assert_eq!(s.doc().unwrap().active_layer_id, dup);
        // Undo drops the duplicate; the source layer is still where it was.
        assert!(s.undo().expect("undo"));
        assert_eq!(s.doc().unwrap().layers.len(), 2);
        assert_eq!(s.doc().unwrap().layers[1].id, id);
        assert_eq!(s.doc().unwrap().layers[1].name, "second");
    }

    #[test]
    fn move_guard_order_is_the_js_one() {
        let mut s = session();
        assert!(!s.history().can_undo());
        let second = add_layer(&mut s, Some("b")).expect("add");
        let before = s.history().undo_label();
        let at = s.doc().unwrap().index_of(&second).expect("present");
        // Same position: no-op, no new history entry.
        assert!(!move_layer(&mut s, &second, at as i64).expect("move"));
        assert_eq!(s.history().undo_label(), before);
        // Out of range below zero: an error, not a no-op.
        assert!(move_layer(&mut s, &second, -1).is_err());
        assert!(move_layer(&mut s, "nope", 0).is_err());
        assert_eq!(s.history().undo_label(), before);
    }

    #[test]
    fn opacity_final_breaks_the_merge_even_when_unchanged() {
        let mut s = session();
        let id = s.doc().unwrap().active_layer_id.clone();
        set_layer_opacity(&mut s, &id, 0.5, false).expect("set");
        set_layer_opacity(&mut s, &id, 0.25, false).expect("set");
        set_layer_opacity(&mut s, &id, 0.25, true).expect("same, final");
        set_layer_opacity(&mut s, &id, 0.75, false).expect("set");
        // Two entries: the merged drag and the post-final change.
        let mut n = 0;
        while s.undo().expect("undo") {
            n += 1;
        }
        assert_eq!(n, 2);
    }

    #[test]
    fn set_active_layer_has_no_history_entry() {
        let mut s = session();
        let first = s.doc().unwrap().active_layer_id.clone();
        let second = add_layer(&mut s, Some("b")).expect("add");
        assert_eq!(s.doc().unwrap().active_layer_id, second);
        let before = s.history().undo_label();
        assert!(set_active_layer(&mut s, &first).expect("set"));
        assert!(
            !set_active_layer(&mut s, &first).expect("same"),
            "selecting the same layer twice is a no-op"
        );
        assert_eq!(
            s.history().undo_label(),
            before,
            "selecting a layer is not undoable"
        );
        assert!(set_active_layer(&mut s, &second).expect("set"));
        // The single entry is the `add_layer` command.
        assert!(s.undo().expect("undo"));
        assert_eq!(s.doc().unwrap().active_layer_id, first);
    }
    #[test]
    fn resize_canvas_skips_the_no_op_and_validates() {
        let mut s = session();
        assert!(!resize_canvas(&mut s, 32, 32).expect("same"));
        assert!(resize_canvas(&mut s, 64, 64).expect("grow"));
        assert_eq!(s.doc().unwrap().canvas.width_px, 64);
        assert!(resize_canvas(&mut s, 33, 32).is_err());
    }

    #[test]
    fn unchanged_properties_are_not_history_entries() {
        let mut s = session();
        let id = s.doc().unwrap().active_layer_id.clone();
        assert!(!set_layer_visible(&mut s, &id, true).expect("same"));
        assert!(!set_layer_locked(&mut s, &id, false).expect("same"));
        assert!(!set_layer_blend(&mut s, &id, "normal").expect("same"));
        assert!(!rename_layer(&mut s, &id, "Layer 1").expect("same"));
        assert!(!s.history().can_undo());
        assert!(set_layer_blend(&mut s, &id, "multiply").expect("change"));
        assert!(s.history().can_undo());
    }
}
