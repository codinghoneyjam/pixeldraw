//! The `Command` trait and the effects vocabulary every command speaks in.
//!
//! Ported from the duck-typed shape of `src/features/layers/commands.js`:
//! `do` / `undo` / `byteSize` / `effects` / an optional `mergeWith`, all stored
//! as `Box<dyn Command>` in the undo stacks. The trait is the one place the JS
//! `typeof` checks become compile-time knowledge:
//!
//! * `typeof cmd.mergeWith === "function"` → [`Command::supports_merge`].
//! * `prev instanceof SetLayerPropCommand` → [`Command::merge_with`], which
//!   downcasts through [`Command::as_any`].
//!
//! The names `apply`/`revert` stand in for the JS `do`/`undo`, because `do` is
//! a Rust keyword.

use crate::errors::EngineResult;
use crate::model::document::Document;

/// Which structural change a command reports to the session.
///
/// The strings are the JS `fx.layers.reason` values: the layers panel switches
/// on them (`panel_layers.js`), so they are frozen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerChangeReason {
    Add,
    Remove,
    Move,
    Prop,
    Active,
}

impl LayerChangeReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Move => "move",
            Self::Prop => "prop",
            Self::Active => "active",
        }
    }
}

/// A pixel change as the renderer's cache-invalidation detail.
///
/// `Chunks` mirrors `{ layerId, chunks }` and `All` mirrors `{ layerId, all: true
/// }`. The JS `p.all === true` test is the only discriminator, which is why this
/// is an enum with two variants rather than one struct with a flag.
#[derive(Debug, Clone, PartialEq)]
pub enum PixelEffect {
    Chunks {
        layer_id: String,
        chunks: Vec<(u32, u32)>,
    },
    All {
        layer_id: String,
    },
}

/// What a command did, so the session can dispatch `layers-changed` /
/// `pixels-changed` without knowing the command's type.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandEffects {
    /// `None` when the JS `effects()` returned `layers: null` (a paint).
    pub layers: Option<LayerEffect>,
    pub pixels: Vec<PixelEffect>,
}

impl CommandEffects {
    /// The `pixels: []` of the pure structure commands.
    pub fn structure(reason: LayerChangeReason, layer_ids: Vec<String>) -> Self {
        Self {
            layers: Some(LayerEffect { reason, layer_ids }),
            pixels: Vec::new(),
        }
    }
}

/// The `fx.layers` payload.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerEffect {
    pub reason: LayerChangeReason,
    pub layer_ids: Vec<String>,
}

/// One undo/redo step.
///
/// `Debug` so the stacks can derive it and a failed expectation prints the
/// command rather than a bare `Box<dyn Command>`.
pub trait Command: std::fmt::Debug {
    /// The undo-stack label, as the JS `this.label`.
    fn label(&self) -> &str;

    /// The JS `this.type` string — `"paint"`, `"add-layer"`, ... Nothing in the
    /// ported tests reads it yet, but the schema/history display will, so the
    /// vocabulary is frozen here rather than re-derived per command.
    fn type_name(&self) -> &'static str;

    /// `typeof cmd.mergeWith === "function"`.
    ///
    /// Only [`crate::model::commands::SetLayerPropCommand`] defines `mergeWith`
    /// in the JS tree, so only it returns `true`; the flag gates whether the
    /// *next* commit is even allowed to try merging.
    fn supports_merge(&self) -> bool {
        false
    }

    /// JS `do(doc)`.
    fn apply(&mut self, doc: &mut Document) -> EngineResult<()>;

    /// JS `undo(doc)`.
    fn revert(&mut self, doc: &mut Document) -> EngineResult<()>;

    /// `changeSetBytes` / the manual chunk accounting — the undo budget's unit.
    fn byte_size(&self) -> usize;

    /// JS `effects()`.
    fn effects(&self) -> CommandEffects;

    /// JS `mergeWith(prev)`, called as `cmd.mergeWith(top)` with `self` the
    /// **new** command and `prev` the one already on the stack. Returns `true`
    /// when `prev` absorbed this step (the JS mutated `prev` in place, which is
    /// what the `&mut dyn Command` parameter models).
    fn merge_with(&mut self, prev: &mut dyn Command) -> bool {
        let _ = prev;
        false
    }

    /// Downcast hook for [`Command::merge_with`]. Implemented by every command
    /// as `self`; `dyn Command` cannot be cast to `dyn Any` without it.
    fn as_any(&mut self) -> &mut dyn std::any::Any;
}
