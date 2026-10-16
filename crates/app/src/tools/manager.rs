//! `tools/tool_manager.js` — tool registry and dispatch.
//!
//! The JS manager subscribed to the session bus itself; here the session bus
//! cannot call back into the tools (`'static FnMut`), so W8 drains the event
//! queue into [`ToolManager::on_model_event`], which forwards to every tool
//! and switches on `Settings{ActiveTool}`. Registration does NOT auto-activate
//! (activation needs a `ToolCx`); W8 calls [`ToolManager::activate_initial`]
//! once after registering the set.

use std::collections::HashMap;

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::model::event::EventDetail;
use pixel_engine::model::{ModelEvent, SettingKey, SettingValue};

use crate::input::{HoverEvent, KeyEvent, ToolEvent};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

pub struct ToolManager {
    tools: HashMap<&'static str, Box<dyn Tool>>,
    active_id: &'static str,
}

impl ToolManager {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            active_id: "pen",
        }
    }

    /// `register` — the id collision rule is last-wins, like the JS `Map.set`.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.id(), tool);
    }

    pub fn get(&self, id: &str) -> Option<&dyn Tool> {
        self.tools.get(id).map(AsRef::as_ref)
    }

    pub fn get_mut<'a>(&'a mut self, id: &str) -> Option<&'a mut (dyn Tool + 'static)> {
        self.tools.get_mut(id).map(|tool| &mut **tool)
    }

    /// `get active` — unregistered id is `INVALID_STATE`, like the JS throw.
    pub fn active(&self) -> EngineResult<&dyn Tool> {
        self.tools
            .get(self.active_id)
            .map(AsRef::as_ref)
            .ok_or_else(|| {
                DrawToolError::invalid_state(format!("unregistered tool {}", self.active_id))
            })
    }

    pub fn active_id(&self) -> &'static str {
        self.active_id
    }

    pub fn set_initial_id(&mut self, id: &'static str) {
        self.active_id = id;
    }

    /// Activate the current tool after the set is registered (W8 boot).
    pub fn activate_initial(&mut self, cx: &mut ToolCx) -> EngineResult<()> {
        let tool = self.tools.get_mut(self.active_id).ok_or_else(|| {
            DrawToolError::invalid_state(format!("unregistered tool {}", self.active_id))
        })?;
        tool.activate(cx);
        Ok(())
    }

    /// `_switchTo` — a live stroke on the previous tool is cancelled first,
    /// then deactivate/activate. Errors never block the switch (JS parity).
    pub fn switch_to(&mut self, next: &'static str, cx: &mut ToolCx) {
        if self.active_id == next {
            return;
        }
        if let Some(prev) = self.tools.get_mut(self.active_id) {
            if cx.session.is_editing() {
                prev.cancel(cx);
            }
            prev.deactivate(cx);
        }
        self.active_id = next;
        if let Some(tool) = self.tools.get_mut(next) {
            tool.activate(cx);
        }
    }

    fn with_active(
        &mut self,
        cx: &mut ToolCx,
        f: impl FnOnce(&mut dyn Tool, &mut ToolCx),
    ) -> EngineResult<()> {
        let tool = self.tools.get_mut(self.active_id).ok_or_else(|| {
            DrawToolError::invalid_state(format!("unregistered tool {}", self.active_id))
        })?;
        f(tool.as_mut(), cx);
        Ok(())
    }

    pub fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) -> EngineResult<()> {
        self.with_active(cx, |t, cx| t.pointer_down(ev, cx))
    }

    pub fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) -> EngineResult<()> {
        self.with_active(cx, |t, cx| t.pointer_move(ev, cx))
    }

    pub fn pointer_up(&mut self, ev: &ToolEvent, cx: &mut ToolCx) -> EngineResult<()> {
        self.with_active(cx, |t, cx| t.pointer_up(ev, cx))
    }

    pub fn hover(&mut self, ev: Option<&HoverEvent>, cx: &mut ToolCx) -> EngineResult<()> {
        self.with_active(cx, |t, cx| t.hover(ev, cx))
    }

    pub fn cancel(&mut self, cx: &mut ToolCx) -> EngineResult<()> {
        self.with_active(cx, |t, cx| t.cancel(cx))
    }

    pub fn key_down(&mut self, ev: &KeyEvent, cx: &mut ToolCx) -> EngineResult<bool> {
        let tool = self.tools.get_mut(self.active_id).ok_or_else(|| {
            DrawToolError::invalid_state(format!("unregistered tool {}", self.active_id))
        })?;
        Ok(tool.key_down(ev, cx))
    }

    pub fn cursor(&self) -> CursorKind {
        self.active()
            .map(|t| t.cursor())
            .unwrap_or(CursorKind::Default)
    }

    pub fn overlay(&self, info: &OverlayInfo, out: &mut Vec<OverlayOp>) {
        if let Ok(tool) = self.active() {
            tool.overlay(info, out);
        }
    }

    /// The W8 queue drain: every tool sees the event (pen clears `last_point`
    /// on document/layer switches), then `ActiveTool` switches the set.
    pub fn on_model_event(&mut self, ev: &ModelEvent, cx: &mut ToolCx) {
        for tool in self.tools.values_mut() {
            tool.on_model_event(ev, cx);
        }
        if let EventDetail::Settings {
            key: SettingKey::ActiveTool,
            value: SettingValue::Mode(id),
        } = &ev.detail
        {
            self.switch_to(id, cx);
        }
    }
}

impl Default for ToolManager {
    fn default() -> Self {
        Self::new()
    }
}
