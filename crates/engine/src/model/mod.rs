//! The document model — the JS `features/document/` + `features/layers/` pair.
//!
//! This is the layer between the drawing kernels and the UI: everything that
//! owns a canvas rather than a pixel. Ported 1:1 from
//! `src/features/document/{document,session,history,settings_validator}.js` and
//! `src/features/layers/{layer,commands,commands_pixel,edit_session,layer_operations,
//! layer_actions}.js`.
//!
//! ```text
//! model/
//! ├── layer.rs          Layer + the property validators        (layer.js)
//! ├── ids.rs            IdGen + the doc counter                (document.js L8-27)
//! ├── canvas.rs         CanvasInfo, background, viewport       (document.js L31-71)
//! ├── document.rs       Document + newDocument                 (document.js L73-242)
//! ├── settings.rs       the nine UI settings + validateSetting (settings_validator.js)
//! ├── command.rs        the Command trait + effects vocabulary (commands.js)
//! ├── commands.rs       Paint/Add/Remove/Move/SetLayerProp     (commands.js)
//! ├── commands_pixel.rs MergeDown + ResizeCanvas               (commands_pixel.js)
//! ├── history.rs        UndoManager — byte budget, merge policy (history.js)
//! ├── event.rs          the seven signals + their observer list (events.js)
//! ├── session.rs        Session — everything glued              (session.js)
//! ├── edit_session.rs   beginEdit / commit / cancel             (edit_session.js)
//! ├── operations.rs     the session-level layer operations      (layer_operations.js)
//! └── actions.rs        the layer-panel action handlers         (layer_actions.js)
//! ```
//!
//! `ids`/`canvas`/`document` are one JS file split three ways so each module
//! stays under the 400-line guide; the seams are the natural ones in that file.
//!
//! **Deliberately not ported**: `panel_layers.js` (UI, Phase 3.5).
//! `settings_validator.js` *was* ported as [`settings`] — it imports only
//! `errors`/`events`/`constants`/`pixel`, so it reaches no DOM and no
//! `localStorage`, which is why nothing had to be deferred.
//!
//! **Layer order is bottom → top**, matching the JS array: the panel draws
//! `layers.reverse()` top-first, and the compositor accumulates in array order,
//! so the last entry paints last (see `composite.rs`).

pub mod actions;
pub mod canvas;
pub mod command;
pub mod commands;
pub mod commands_pixel;
pub mod document;
pub mod edit_session;
pub mod event;
pub mod history;
pub mod ids;
pub mod layer;
pub mod operations;
pub mod session;
pub mod settings;

pub use canvas::{CanvasInfo, Viewport, validate_background, validate_viewport};
pub use command::{Command, CommandEffects, LayerChangeReason, LayerEffect, PixelEffect};
pub use commands::{
    AddLayerCommand, MoveLayerCommand, PaintCommand, RemoveLayerCommand, SetLayerPropCommand,
};
pub use commands_pixel::{MergeDownCommand, ResizeCanvasCommand};
pub use document::{Document, DocumentSpec, NewDocumentOptions, new_document};
pub use edit_session::{EditHandle, EditOptions};
pub use event::{EventBus, EventDetail, ModelEvent, Subscription};
pub use history::{HistoryNotify, HistoryState, NotifyKind, UndoManager};
pub use ids::{IdGen, next_doc_id};
pub use layer::{LAYER_PROP_FIELDS, Layer, LayerField, LayerPropValue};
pub use operations::{
    add_layer, duplicate_layer, insert_layer, merge_down, move_layer, remove_layer, resize_canvas,
    set_active_layer, set_layer_blend, set_layer_locked, set_layer_opacity, set_layer_visible,
};
pub use session::Session;
pub use settings::{SettingInput, SettingKey, SettingValue, Settings, TOOL_IDS, validate_setting};
