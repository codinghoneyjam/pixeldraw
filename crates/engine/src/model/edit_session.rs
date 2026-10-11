//! The stroke edit handle — `beginEdit` / `flush` / `commit` / `cancel`.
//!
//! Ported 1:1 from `src/features/layers/edit_session.js`. The handle owns the
//! [`crate::chunkstore::PixelWriter`] for one stroke and a borrow of the
//! session's non-document state, so `commit` can record the resulting
//! [`PaintCommand`] and emit the change signals without the caller having to
//! remember the order.
//!
//! **The one thing the type system upgrades.** The JS kept
//! `session._edit` and `commit`/`cancel` each began with
//! `if (session._edit !== edit) throw INVALID_STATE`. Here `commit` and
//! `cancel` *consume* the handle, so a second call is a compile error rather
//! than a runtime one.

use crate::chunkstore::PixelWriter;
use crate::errors::{DrawToolError, EngineResult};
use crate::model::commands::PaintCommand;
use crate::model::event::ModelEvent;
use crate::model::session::SessionCore;

/// The `beginEdit(session, opts)` argument.
#[derive(Debug, Clone, Default)]
pub struct EditOptions {
    pub layer_id: Option<String>,
    /// Defaults to `"연필"`, the JS destructuring default.
    pub label: Option<String>,
}

impl EditOptions {
    pub fn on_active_layer() -> Self {
        Self::default()
    }

    pub fn on_layer(layer_id: &str) -> Self {
        Self {
            layer_id: Some(layer_id.to_string()),
            label: None,
        }
    }

    pub fn labeled(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }
}

/// One in-progress stroke.
#[derive(Debug)]
pub struct EditHandle<'a> {
    /// Emptied by `commit`/`cancel` before the handle is consumed.
    writer: Option<PixelWriter<'a>>,
    layer_id: String,
    label: String,
    core: &'a mut SessionCore,
}

impl<'a> EditHandle<'a> {
    pub(crate) fn new(
        writer: PixelWriter<'a>,
        layer_id: String,
        label: String,
        core: &'a mut SessionCore,
    ) -> Self {
        Self {
            writer: Some(writer),
            layer_id,
            label,
            core,
        }
    }

    /// `edit.writer` — the stroke sink the shape tools hand to `apply_shape`,
    /// the pen hands to `strokeSegment`, and the flood fill stamps through.
    ///
    /// `None` after the handle is finished: `PixelWriter::finish` consumes the
    /// value it takes, so the slot is emptied first. Reaching that state means
    /// writing into a stroke that has already ended, which is what the JS
    /// `"edit is not active"` error covered.
    pub fn writer(&mut self) -> Option<&mut PixelWriter<'a>> {
        self.writer.as_mut()
    }

    /// The label the JS `commit` would fall back to.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The layer being drawn into.
    pub fn layer_id(&self) -> &str {
        &self.layer_id
    }

    /// `session._edit !== null` while the handle lives.
    pub fn is_active(&self) -> bool {
        self.core.editing
    }

    /// `edit.flush()` — drain the dirty list so a mid-stroke render can pick it
    /// up, without closing the writer.
    pub fn flush(&mut self) -> Vec<(u32, u32)> {
        let Some(writer) = self.writer.as_mut() else {
            return Vec::new();
        };
        let dirty = writer.take_dirty();
        if !dirty.is_empty() {
            self.core.bus.emit(ModelEvent::pixels_changed(
                self.layer_id.clone(),
                dirty.clone(),
            ));
        }
        dirty
    }

    /// `edit.commit(commitLabel)` — close the stroke and record it.
    ///
    /// Returns `false` when nothing changed, exactly as the JS returned `false`
    /// for a null changeset: no history entry, no `paint` undo step.
    pub fn commit(mut self, commit_label: Option<&str>) -> EngineResult<bool> {
        let Some(mut writer) = self.writer.take() else {
            return Err(DrawToolError::invalid_state("edit is not active"));
        };
        // The JS read `takeDirty()` *after* `finish()`. The Rust `finish`
        // consumes the writer, so the list is drained first; `finish` never
        // touches it, so the two orders return the same list.
        let dirty = writer.take_dirty();
        let cs = writer.finish();
        self.core.editing = false;
        if !dirty.is_empty() {
            self.core
                .bus
                .emit(ModelEvent::pixels_changed(self.layer_id.clone(), dirty));
        }
        let Some(cs) = cs else {
            return Ok(false);
        };
        let label = commit_label
            .map(str::to_string)
            .unwrap_or_else(|| self.label.clone());
        let effects = self.core.history.record(PaintCommand::new(cs, label))?;
        self.core.dispatch_effects(&effects);
        Ok(true)
    }

    /// `edit.cancel()` — `discard` the writer's before-images and report what
    /// changed so the view can repaint.
    pub fn cancel(mut self) {
        let Some(writer) = self.writer.take() else {
            return;
        };
        let dirty = writer.discard();
        self.core.editing = false;
        if !dirty.is_empty() {
            self.core
                .bus
                .emit(ModelEvent::pixels_changed(self.layer_id.clone(), dirty));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_options_default_to_the_active_layer_and_the_default_label() {
        let o = EditOptions::default();
        assert!(o.layer_id.is_none());
        assert!(o.label.is_none());
        let o = EditOptions::on_layer("layer-0007").labeled("연필");
        assert_eq!(o.layer_id.as_deref(), Some("layer-0007"));
        assert_eq!(o.label.as_deref(), Some("연필"));
    }

    #[test]
    fn commit_effect_shape_matches_the_js_dispatch() {
        // `PaintCommand::effects` is what the JS `_dispatchEffects` consumed.
        let cs = crate::chunkstore::Changeset {
            layer_id: "layer-0001".into(),
            chunks: vec![(0, 0, None, None)],
        };
        let cmd = PaintCommand::with_default_label(cs);
        let fx = crate::model::command::Command::effects(&cmd);
        assert!(fx.layers.is_none());
        assert_eq!(fx.pixels.len(), 1);
    }
}
