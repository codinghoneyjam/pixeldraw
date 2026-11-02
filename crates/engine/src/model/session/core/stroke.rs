//! Pixel editing and stroke lifecycle for a session.

use super::*;

impl Session {
    pub fn begin_edit(&mut self, opts: EditOptions) -> EngineResult<EditHandle<'_>> {
        let Self { doc, core } = self;
        let document = doc.as_mut().ok_or_else(no_document)?;
        if core.editing || core.stroke.is_some() {
            return Err(DrawToolError::invalid_state("already editing"));
        }
        let layer_id = opts
            .layer_id
            .clone()
            .unwrap_or_else(|| document.active_layer_id.clone());
        let layer = document.get_layer_mut(&layer_id)?;
        if layer.locked {
            // The JS notified *and* threw; the status message is what the option
            // bar's warning comes from, so it must be emitted before the error
            core.bus.emit(ModelEvent::status_message(
                "warn",
                "레이어가 잠겨 있습니다",
                crate::errors::CODE_LAYER_LOCKED,
            ));
            return Err(DrawToolError::layer_locked());
        }
        if !layer.visible {
            core.bus.emit(ModelEvent::status_message(
                "warn",
                "레이어가 숨겨져 있습니다",
                crate::errors::CODE_LAYER_HIDDEN,
            ));
            return Err(DrawToolError::layer_hidden());
        }
        let writer = crate::chunkstore::PixelWriter::new(&mut layer.store, layer.id.clone());
        let label = opts.label.unwrap_or_else(|| "연필".to_string());
        core.editing = true;
        Ok(EditHandle::new(writer, layer_id, label, core))
    }

    pub fn stroke_begin(&mut self, opts: EditOptions) -> EngineResult<()> {
        let Self { doc, core } = self;
        let document = doc.as_mut().ok_or_else(no_document)?;
        if core.editing || core.stroke.is_some() {
            return Err(DrawToolError::invalid_state("already editing"));
        }
        let layer_id = opts
            .layer_id
            .clone()
            .unwrap_or_else(|| document.active_layer_id.clone());
        let layer = document.get_layer_mut(&layer_id)?;
        if layer.locked {
            core.bus.emit(ModelEvent::status_message(
                "warn",
                "레이어가 잠겨 있습니다",
                crate::errors::CODE_LAYER_LOCKED,
            ));
            return Err(DrawToolError::layer_locked());
        }
        if !layer.visible {
            core.bus.emit(ModelEvent::status_message(
                "warn",
                "레이어가 숨겨져 있습니다",
                crate::errors::CODE_LAYER_HIDDEN,
            ));
            return Err(DrawToolError::layer_hidden());
        }

        core.editing = true;
        core.stroke = Some(ActiveStroke {
            layer_id: layer_id.clone(),
            label: opts.label.unwrap_or_else(|| "연필".to_string()),
            state: StrokeState {
                layer_id: layer_id.clone(),
                before: BTreeMap::new(),
                dirty: BTreeSet::new(),
                last_key: None,
            },
        });
        Ok(())
    }

    pub fn stroke_with<R>(&mut self, f: impl FnOnce(&mut PixelWriter<'_>) -> R) -> EngineResult<R> {
        let stroke = self
            .core
            .stroke
            .as_ref()
            .cloned()
            .ok_or_else(|| DrawToolError::invalid_state("edit is not active"))?;
        let layer_id = stroke.layer_id.clone();
        let document = self.doc.as_mut().ok_or_else(no_document)?;
        let layer = document.get_layer_mut(&layer_id).inspect_err(|_| {
            self.core.editing = false;
            self.core.stroke = None;
        })?;

        let mut writer = PixelWriter::resume(&mut layer.store, stroke.state.clone());
        let out = f(&mut writer);
        let state = writer.suspend();
        self.core.stroke = Some(ActiveStroke {
            layer_id: layer_id.clone(),
            label: stroke.label,
            state,
        });
        Ok(out)
    }

    pub fn stroke_flush(&mut self) -> Vec<(u32, u32)> {
        let Some(stroke) = self.core.stroke.clone() else {
            return Vec::new();
        };
        let Some(document) = self.doc.as_mut() else {
            self.core.editing = false;
            self.core.stroke = None;
            return Vec::new();
        };
        let Ok(layer) = document.get_layer_mut(&stroke.layer_id) else {
            self.core.editing = false;
            self.core.stroke = None;
            return Vec::new();
        };

        let mut writer = PixelWriter::resume(&mut layer.store, stroke.state);
        let dirty = writer.take_dirty();
        if !dirty.is_empty() {
            self.core.bus.emit(ModelEvent::pixels_changed(
                stroke.layer_id.clone(),
                dirty.clone(),
            ));
        }
        let state = writer.suspend();
        self.core.stroke = Some(ActiveStroke {
            layer_id: stroke.layer_id.clone(),
            label: stroke.label,
            state,
        });
        dirty
    }

    pub fn stroke_commit(&mut self, label: Option<&str>) -> EngineResult<bool> {
        let Some(stroke) = self.core.stroke.clone() else {
            return Err(DrawToolError::invalid_state("edit is not active"));
        };
        let document = self.doc.as_mut().ok_or_else(no_document)?;
        let layer = document.get_layer_mut(&stroke.layer_id).map_err(|_| {
            self.core.editing = false;
            self.core.stroke = None;
            DrawToolError::invalid_state("edit is not active")
        })?;

        let mut writer = PixelWriter::resume(&mut layer.store, stroke.state);
        let dirty = writer.take_dirty();
        let cs = writer.finish();
        self.core.editing = false;
        self.core.stroke = None;
        if !dirty.is_empty() {
            self.core
                .bus
                .emit(ModelEvent::pixels_changed(stroke.layer_id.clone(), dirty));
        }
        let Some(cs) = cs else {
            return Ok(false);
        };
        let label_value = label
            .map(str::to_string)
            .unwrap_or_else(|| stroke.label.clone());
        let effects = self
            .core
            .history
            .record(crate::model::commands::PaintCommand::new(cs, label_value))?;
        self.core.dispatch_effects(&effects);
        Ok(true)
    }

    pub fn stroke_cancel(&mut self) {
        let Some(stroke) = self.core.stroke.clone() else {
            return;
        };
        let Some(document) = self.doc.as_mut() else {
            self.core.editing = false;
            self.core.stroke = None;
            return;
        };
        let Ok(layer) = document.get_layer_mut(&stroke.layer_id) else {
            self.core.editing = false;
            self.core.stroke = None;
            return;
        };

        let writer = PixelWriter::resume(&mut layer.store, stroke.state);
        let dirty = writer.discard();
        self.core.editing = false;
        self.core.stroke = None;
        if !dirty.is_empty() {
            self.core
                .bus
                .emit(ModelEvent::pixels_changed(stroke.layer_id.clone(), dirty));
        }
    }
}
