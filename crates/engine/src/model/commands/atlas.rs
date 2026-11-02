//! Undoable document atlas-layout changes.

use crate::atlas::AtlasLayout;
use crate::errors::{DrawToolError, EngineResult};
use crate::model::command::{Command, CommandEffects};
use crate::model::document::Document;

#[derive(Debug, Clone)]
pub struct SetAtlasLayoutCommand {
    pub before: Option<AtlasLayout>,
    pub after: Option<AtlasLayout>,
}

impl SetAtlasLayoutCommand {
    pub fn new(before: Option<AtlasLayout>, after: Option<AtlasLayout>) -> Self {
        Self { before, after }
    }

    fn set(doc: &mut Document, layout: &Option<AtlasLayout>) -> EngineResult<()> {
        if let Some(layout) = layout {
            layout.validate(doc).map_err(|e| {
                DrawToolError::invalid_state(format!("invalid atlas layout: {}", e.message))
            })?;
        }
        doc.atlas = layout.clone();
        Ok(())
    }
}

impl Command for SetAtlasLayoutCommand {
    fn label(&self) -> &str {
        "아틀라스 레이아웃 변경"
    }

    fn type_name(&self) -> &'static str {
        "set-atlas-layout"
    }

    fn apply(&mut self, doc: &mut Document) -> EngineResult<()> {
        Self::set(doc, &self.after)
    }

    fn revert(&mut self, doc: &mut Document) -> EngineResult<()> {
        Self::set(doc, &self.before)
    }

    fn byte_size(&self) -> usize {
        serde_json::to_vec(&self.before).map_or(0, |v| v.len())
            + serde_json::to_vec(&self.after).map_or(0, |v| v.len())
    }

    fn effects(&self) -> CommandEffects {
        CommandEffects::default()
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
