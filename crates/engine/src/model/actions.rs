//! The layer-panel action handlers.
//!
//! Ported 1:1 from `src/features/layers/layer_actions.js`. Each one targets the
//! **active** layer, so the name is the whole contract: `layerDown` on the
//! bottom layer asks for `-1` and gets an `OUT_OF_RANGE` error, and `layerUp`
//! on the top layer does the same at the other end. The JS propagated those
//! errors out of the panel handler, so this port does too.

use crate::errors::EngineResult;
use crate::model::session::Session;

/// `layerAdd(session)`.
pub fn layer_add(session: &mut Session) -> EngineResult<String> {
    session.add_layer(None)
}

/// `layerDuplicate(session)`.
pub fn layer_duplicate(session: &mut Session) -> EngineResult<String> {
    let id = session.require_doc()?.active_layer_id.clone();
    session.duplicate_layer(&id)
}

/// `layerRemove(session)`.
pub fn layer_remove(session: &mut Session) -> EngineResult<()> {
    let id = session.require_doc()?.active_layer_id.clone();
    session.remove_layer(&id)
}

/// `layerMergeDown(session)`.
pub fn layer_merge_down(session: &mut Session) -> EngineResult<()> {
    let id = session.require_doc()?.active_layer_id.clone();
    session.merge_down(&id)
}

/// `layerUp(session)` — `activeIndex + 1`.
pub fn layer_up(session: &mut Session) -> EngineResult<bool> {
    let (id, at) = {
        let doc = session.require_doc()?;
        (doc.active_layer_id.clone(), doc.active_index() + 1)
    };
    session.move_layer(&id, at as i64)
}

/// `layerDown(session)` — `activeIndex - 1`, which is `-1` on the bottom layer.
pub fn layer_down(session: &mut Session) -> EngineResult<bool> {
    let at = {
        let doc = session.require_doc()?;
        doc.active_index() as i64 - 1
    };
    let id = session.require_doc()?.active_layer_id.clone();
    session.move_layer(&id, at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::document::NewDocumentOptions;

    fn session() -> Session {
        let mut s = Session::new();
        s.new_document(NewDocumentOptions::default().size(32, 32))
            .expect("new");
        s
    }

    #[test]
    fn up_and_down_move_the_active_layer() {
        let mut s = session();
        let bottom = s.doc().unwrap().active_layer_id.clone();
        let top = session_add(&mut s);
        // Top layer: `down` moves it to the bottom, `up` is out of range.
        assert!(layer_down(&mut s).expect("down"));
        assert_eq!(s.doc().unwrap().layers[0].id, top);
        assert!(layer_up(&mut s).expect("up"));
        assert_eq!(s.doc().unwrap().layers[1].id, top);
        assert!(
            layer_up(&mut s).is_err(),
            "moving past the top is OUT_OF_RANGE, like the JS"
        );
        assert_eq!(s.doc().unwrap().active_layer_id, top);
        s.set_active_layer(&bottom).expect("active");
        assert!(
            layer_down(&mut s).is_err(),
            "moving past the bottom asks for -1"
        );
    }

    fn session_add(s: &mut Session) -> String {
        s.add_layer(Some("top")).expect("add")
    }

    #[test]
    fn add_duplicate_and_remove_target_the_active_layer() {
        let mut s = session();
        let bottom = s.doc().unwrap().active_layer_id.clone();
        let source = layer_add(&mut s).expect("add");
        let dup = layer_duplicate(&mut s).expect("dup");
        assert_eq!(s.doc().unwrap().layers.len(), 3);
        assert_eq!(s.doc().unwrap().active_layer_id, dup);
        assert_eq!(s.doc().unwrap().layers[0].id, bottom);
        assert_eq!(s.doc().unwrap().layers[1].id, source);
        layer_remove(&mut s).expect("remove");
        assert_eq!(s.doc().unwrap().layers.len(), 2);
        // `_remove` re-points the active layer at the one that slid into the
        // hole — the duplicate's source, not the bottom layer.
        assert_eq!(s.doc().unwrap().active_layer_id, source);
        assert_ne!(s.doc().unwrap().active_layer_id, dup);
        layer_merge_down(&mut s).expect("merge");
        assert_eq!(s.doc().unwrap().layers.len(), 1);
    }
}
