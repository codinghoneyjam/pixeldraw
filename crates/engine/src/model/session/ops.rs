//! The session's layer-operation surface — one-line delegates to
//! [`crate::model::operations`], exactly as `session.js` re-exposed
//! `layer_operations.js`'s twelve functions.

use crate::errors::EngineResult;

use super::core::Session;

impl Session {
    pub fn add_layer(&mut self, name: Option<&str>) -> EngineResult<String> {
        crate::model::operations::add_layer(self, name)
    }

    pub fn duplicate_layer(&mut self, id: &str) -> EngineResult<String> {
        crate::model::operations::duplicate_layer(self, id)
    }

    pub fn remove_layer(&mut self, id: &str) -> EngineResult<()> {
        crate::model::operations::remove_layer(self, id)
    }

    pub fn move_layer(&mut self, id: &str, to_index: i64) -> EngineResult<bool> {
        crate::model::operations::move_layer(self, id, to_index)
    }

    pub fn merge_down(&mut self, id: &str) -> EngineResult<()> {
        crate::model::operations::merge_down(self, id)
    }

    pub fn insert_layer(
        &mut self,
        layer: crate::model::layer::Layer,
        index: Option<usize>,
    ) -> EngineResult<String> {
        crate::model::operations::insert_layer(self, layer, index)
    }

    pub fn resize_canvas(&mut self, w: u32, h: u32) -> EngineResult<bool> {
        crate::model::operations::resize_canvas(self, w, h)
    }

    pub fn rename_layer(&mut self, id: &str, name: &str) -> EngineResult<bool> {
        crate::model::operations::rename_layer(self, id, name)
    }

    pub fn set_layer_visible(&mut self, id: &str, visible: bool) -> EngineResult<bool> {
        crate::model::operations::set_layer_visible(self, id, visible)
    }

    pub fn set_layer_locked(&mut self, id: &str, locked: bool) -> EngineResult<bool> {
        crate::model::operations::set_layer_locked(self, id, locked)
    }

    /// `setLayerOpacity(id, value, { final })` — see
    /// [`crate::model::operations::set_layer_opacity`] for what `final` means.
    pub fn set_layer_opacity(&mut self, id: &str, value: f64, final_: bool) -> EngineResult<bool> {
        crate::model::operations::set_layer_opacity(self, id, value, final_)
    }

    pub fn set_layer_blend(&mut self, id: &str, value: &'static str) -> EngineResult<bool> {
        crate::model::operations::set_layer_blend(self, id, value)
    }

    pub fn set_active_layer(&mut self, id: &str) -> EngineResult<bool> {
        crate::model::operations::set_active_layer(self, id)
    }
}
