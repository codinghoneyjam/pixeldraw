//! The seven signals and the observer list that replaces `EventTarget`.
//!
//! The JS `Session` extended `EventTarget` and dispatched `CustomEvent`s; this
//! module holds the same seven names (from [`crate::events`]) with `enum`
//! payloads instead of `detail` bags, plus a subscriber list. The engine stays
//! DOM-free: an app panel subscribes with a closure rather than a listener
//! function object that has to be removed by identity.
//!
//! Listeners are **infallible by signature and fallible by return value**:
//! returning `Err` is how a Rust listener models a JS one that threw.
//! [`EventBus::emit`] drops the error and keeps dispatching, matching the JS
//! `dispatchEvent` (which does not stop at a throwing listener at all) and the
//! history's explicit `try/catch` + `console.error`.

use std::collections::BTreeMap;
use std::fmt;

use crate::events::{
    EVENT_DOCUMENT_REPLACED, EVENT_HISTORY_CHANGED, EVENT_LAYERS_CHANGED, EVENT_PIXELS_CHANGED,
    EVENT_SETTINGS_CHANGED, EVENT_STATUS_MESSAGE, EVENT_TOOL_STATE,
};
use crate::model::command::CommandEffects;
use crate::model::history::HistoryState;
use crate::model::settings::{SettingKey, SettingValue};

/// A subscription handle. Dropping it does **not** unsubscribe, because the JS
/// had to return a removal closure for that; call [`EventBus::unsubscribe`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription {
    id: usize,
}

impl Subscription {
    pub(crate) fn new(id: usize) -> Self {
        Self { id }
    }

    pub(crate) fn id(&self) -> usize {
        self.id
    }
}

/// A listener. `Err` means "this listener threw" — see the module docs.
pub type Listener = Box<dyn FnMut(&ModelEvent) -> crate::errors::EngineResult<()>>;

/// One dispatched event: the frozen name plus its payload.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEvent {
    pub name: &'static str,
    pub detail: EventDetail,
}

impl ModelEvent {
    /// `DOCUMENT_REPLACED` — the JS detail carried the document itself, which
    /// the engine hands back through `Session::doc`.
    pub fn document_replaced() -> Self {
        Self {
            name: EVENT_DOCUMENT_REPLACED,
            detail: EventDetail::DocumentReplaced,
        }
    }

    pub fn layers_changed(
        reason: crate::model::command::LayerChangeReason,
        layer_ids: Vec<String>,
    ) -> Self {
        Self {
            name: EVENT_LAYERS_CHANGED,
            detail: EventDetail::Layers { reason, layer_ids },
        }
    }

    /// `{ layerId, chunks }`.
    pub fn pixels_changed(layer_id: impl Into<String>, chunks: Vec<(u32, u32)>) -> Self {
        Self {
            name: EVENT_PIXELS_CHANGED,
            detail: EventDetail::PixelsChunks {
                layer_id: layer_id.into(),
                chunks,
            },
        }
    }

    /// `{ layerId, all: true }`.
    pub fn pixels_all_changed(layer_id: impl Into<String>) -> Self {
        Self {
            name: EVENT_PIXELS_CHANGED,
            detail: EventDetail::PixelsAll {
                layer_id: layer_id.into(),
            },
        }
    }

    pub fn history_changed(state: HistoryState) -> Self {
        Self {
            name: EVENT_HISTORY_CHANGED,
            detail: EventDetail::History(state),
        }
    }

    pub fn settings_changed(key: SettingKey, value: SettingValue) -> Self {
        Self {
            name: EVENT_SETTINGS_CHANGED,
            detail: EventDetail::Settings { key, value },
        }
    }

    pub fn status_message(
        level: &'static str,
        text: impl Into<String>,
        code: &'static str,
    ) -> Self {
        Self {
            name: EVENT_STATUS_MESSAGE,
            detail: EventDetail::Status {
                level,
                text: text.into(),
                code,
            },
        }
    }

    pub fn tool_state(tool: impl Into<String>, pending: impl Into<String>) -> Self {
        Self {
            name: EVENT_TOOL_STATE,
            detail: EventDetail::ToolState {
                tool: tool.into(),
                pending: pending.into(),
            },
        }
    }

    /// The `layers-changed` and `pixels-changed` events a command's effects
    /// produce, in the JS dispatch order: layers first, then each pixel entry.
    pub fn from_effects(effects: &CommandEffects) -> Vec<Self> {
        let mut out = Vec::with_capacity(1 + effects.pixels.len());
        if let Some(layers) = &effects.layers {
            out.push(Self::layers_changed(
                layers.reason,
                layers.layer_ids.clone(),
            ));
        }
        for pixel in &effects.pixels {
            out.push(match pixel {
                crate::model::command::PixelEffect::Chunks { layer_id, chunks } => {
                    Self::pixels_changed(layer_id.clone(), chunks.clone())
                }
                crate::model::command::PixelEffect::All { layer_id } => {
                    Self::pixels_all_changed(layer_id.clone())
                }
            });
        }
        out
    }
}

/// The `CustomEvent.detail` payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum EventDetail {
    DocumentReplaced,
    Layers {
        reason: crate::model::command::LayerChangeReason,
        layer_ids: Vec<String>,
    },
    PixelsChunks {
        layer_id: String,
        chunks: Vec<(u32, u32)>,
    },
    PixelsAll {
        layer_id: String,
    },
    History(HistoryState),
    Settings {
        key: SettingKey,
        value: SettingValue,
    },
    Status {
        level: &'static str,
        text: String,
        code: &'static str,
    },
    ToolState {
        tool: String,
        /// The tool's pending state, serialized. `shape.js` compares the
        /// serialization for its own change detection, so the string is the
        /// contract; Phase 2/3 owns its type.
        pending: String,
    },
}

/// The observer list.
///
/// A `BTreeMap` keyed by an increasing id so iteration is in subscription order
/// and removing one does not shift the others' ids, which is the one thing the
/// JS `indexOf`-based `removeEventListener` had to be careful about.
#[derive(Default)]
pub struct EventBus {
    listeners: BTreeMap<usize, Listener>,
    next_id: usize,
}

impl fmt::Debug for EventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBus")
            .field("listeners", &self.listeners.len())
            .finish()
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(
        &mut self,
        f: impl FnMut(&ModelEvent) -> crate::errors::EngineResult<()> + 'static,
    ) -> Subscription {
        let id = self.next_id;
        self.next_id += 1;
        self.listeners.insert(id, Box::new(f));
        Subscription::new(id)
    }

    pub fn unsubscribe(&mut self, sub: Subscription) {
        self.listeners.remove(&sub.id());
    }

    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }

    pub fn emit(&mut self, event: ModelEvent) {
        for listener in self.listeners.values_mut() {
            let _ = listener(&event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::command::{LayerChangeReason, PixelEffect};

    #[test]
    fn from_effects_puts_layers_before_pixels() {
        let fx = CommandEffects {
            layers: Some(crate::model::command::LayerEffect {
                reason: LayerChangeReason::Add,
                layer_ids: vec!["layer-0001".into()],
            }),
            pixels: vec![
                PixelEffect::All {
                    layer_id: "layer-0001".into(),
                },
                PixelEffect::Chunks {
                    layer_id: "layer-0002".into(),
                    chunks: vec![(0, 0), (1, 1)],
                },
            ],
        };
        let events = ModelEvent::from_effects(&fx);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].name, EVENT_LAYERS_CHANGED);
        assert_eq!(events[1].name, EVENT_PIXELS_CHANGED);
        assert_eq!(events[2].name, EVENT_PIXELS_CHANGED);
        assert!(matches!(events[1].detail, EventDetail::PixelsAll { .. }));
        assert!(matches!(events[2].detail, EventDetail::PixelsChunks { .. }));
    }

    #[test]
    fn emit_skips_a_throwing_listener() {
        let mut bus = EventBus::new();
        let hits = std::rc::Rc::new(std::cell::RefCell::new(Vec::<&'static str>::new()));
        let a = hits.clone();
        bus.subscribe(move |e| {
            a.borrow_mut().push(e.name);
            Ok(())
        });
        bus.subscribe(|_| Err(crate::errors::DrawToolError::invalid_state("boom")));
        let b = hits.clone();
        bus.subscribe(move |e| {
            b.borrow_mut().push(e.name);
            Ok(())
        });
        bus.emit(ModelEvent::document_replaced());
        assert_eq!(
            &*hits.borrow(),
            &[EVENT_DOCUMENT_REPLACED, EVENT_DOCUMENT_REPLACED]
        );
        assert_eq!(bus.listener_count(), 3);
    }

    #[test]
    fn unsubscribe_by_handle_removes_only_that_listener() {
        let mut bus = EventBus::new();
        let hits = std::rc::Rc::new(std::cell::RefCell::new(0usize));
        let a = hits.clone();
        let first = bus.subscribe(move |_| {
            *a.borrow_mut() += 1;
            Ok(())
        });
        let b2 = hits.clone();
        bus.subscribe(move |_| {
            *b2.borrow_mut() += 10;
            Ok(())
        });
        bus.emit(ModelEvent::document_replaced());
        assert_eq!(*hits.borrow(), 11);
        bus.unsubscribe(first);
        bus.emit(ModelEvent::document_replaced());
        assert_eq!(*hits.borrow(), 21);
    }

    #[test]
    fn event_names_come_from_the_frozen_catalog() {
        for e in [
            ModelEvent::document_replaced(),
            ModelEvent::layers_changed(LayerChangeReason::Prop, vec![]),
            ModelEvent::pixels_changed("layer-0001", vec![]),
            ModelEvent::pixels_all_changed("layer-0001"),
            ModelEvent::history_changed(HistoryState::default()),
            ModelEvent::settings_changed(SettingKey::PenSize, SettingValue::Int(1)),
            ModelEvent::status_message("warn", "x", "LAYER_LOCKED"),
            ModelEvent::tool_state("rect", "{}"),
        ] {
            assert!(
                crate::events::EVENTS.contains(&e.name),
                "{} is outside the catalog",
                e.name
            );
        }
    }
}
