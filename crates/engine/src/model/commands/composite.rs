//! `CompositeCommand` — several commands as one undo step.
//!
//! Ported from the feature-expansion plan M0-5. Used by F01 (atlas slot
//! edits), F02 (layer presets) and F11 (group creation) so a multi-step
//! structural change reverts with a single Ctrl+Z.

use crate::errors::EngineResult;
use crate::model::command::{Command, CommandEffects, LayerEffect, PixelEffect};
use crate::model::document::Document;

/// A sequence of commands applied and reverted as one unit.
pub struct CompositeCommand {
    label: String,
    commands: Vec<Box<dyn Command>>,
}

impl CompositeCommand {
    pub fn new(label: impl Into<String>, commands: Vec<Box<dyn Command>>) -> Self {
        Self {
            label: label.into(),
            commands,
        }
    }

    /// Number of inner commands (test/inspection helper).
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl std::fmt::Debug for CompositeCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeCommand")
            .field("label", &self.label)
            .field("len", &self.commands.len())
            .finish()
    }
}

impl Command for CompositeCommand {
    fn label(&self) -> &str {
        &self.label
    }

    fn type_name(&self) -> &'static str {
        "composite"
    }

    fn apply(&mut self, doc: &mut Document) -> EngineResult<()> {
        // Apply in order; on failure revert the ones already applied.
        for (i, cmd) in self.commands.iter_mut().enumerate() {
            if let Err(e) = cmd.apply(doc) {
                for cmd in self.commands[..i].iter_mut().rev() {
                    let _ = cmd.revert(doc);
                }
                return Err(e);
            }
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> EngineResult<()> {
        // Revert in reverse order; keep going on error but return the first.
        let mut first_err = None;
        for cmd in self.commands.iter_mut().rev() {
            if let Err(e) = cmd.revert(doc)
                && first_err.is_none()
            {
                first_err = Some(e);
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn byte_size(&self) -> usize {
        self.commands.iter().map(|c| c.byte_size()).sum()
    }

    fn effects(&self) -> CommandEffects {
        // Merge layer effects: keep the reason of the last inner command that
        // reported layers, and union the layer ids. Pixels are concatenated.
        let mut layers: Option<LayerEffect> = None;
        let mut pixels: Vec<PixelEffect> = Vec::new();
        for cmd in &self.commands {
            let e = cmd.effects();
            if let Some(le) = e.layers {
                match &mut layers {
                    None => layers = Some(le),
                    Some(existing) => {
                        existing.reason = le.reason;
                        for id in le.layer_ids {
                            if !existing.layer_ids.contains(&id) {
                                existing.layer_ids.push(id);
                            }
                        }
                    }
                }
            }
            pixels.extend(e.pixels);
        }
        CommandEffects { layers, pixels }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
