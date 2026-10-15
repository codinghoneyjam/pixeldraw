//! The renderer's four event handlers — JS `_onPixels`/`_onLayers`/`_onSettings`/
//! `_onDocReplaced`.
//!
//! The JS renderer subscribed to four of the seven session events and ignored the
//! rest, which is why [`CompCache::apply_event`] returns "needs a frame" instead
//! of a unit: everything a subscriber answered with `requestRender()` is `true`
//! here, and the invalidation side is a separate decision.

use crate::model::command::LayerChangeReason;
use crate::model::event::{EventDetail, ModelEvent};
use crate::model::settings::SettingKey;

use super::cache::CompCache;

impl CompCache {
    /// Feed one `ModelEvent`; returns whether the frame must be re-rendered.
    ///
    /// The event table is JS-exact (docs/roadmap/phase2.yaml W3
    /// `verified_js_behaviour.events`); the JS handlers were named after the
    /// event they consumed, so each arm below carries that name.
    pub fn apply_event(&mut self, ev: &ModelEvent) -> bool {
        match &ev.detail {
            // `_onDocReplaced` → `_setDocument(doc)`: `ensureComp()` (a fresh
            // buffer even at equal size) then `invalidateAll()`.
            EventDetail::DocumentReplaced => {
                self.request_reensure();
                self.invalidate_all();
                true
            }
            // `_onPixels`: `detail.all === true` versus a chunks array — both
            // always asked for a frame.
            EventDetail::PixelsAll { .. } => {
                self.invalidate_all();
                true
            }
            EventDetail::PixelsChunks { chunks, .. } => {
                self.invalidate_chunks(chunks);
                true
            }
            // `_onLayers`: `reason === "active"` returned early. The JS also
            // returned early on `reason === "prop" && d.field === "name"`, but
            EventDetail::Layers { reason, .. } => match reason {
                LayerChangeReason::Active => false,
                _ => {
                    self.invalidate_all();
                    true
                }
            },
            // `_onSettings`: only the grid changes the frame, and it changes no
            // pixels, so it never invalidates.
            EventDetail::Settings { key, .. } => *key == SettingKey::GridMode,
            EventDetail::History(_)
            | EventDetail::Status { .. }
            | EventDetail::ToolState { .. } => false,
        }
    }
}
