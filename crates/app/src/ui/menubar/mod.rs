//! Menubar — port of `src/ui/menubar/**`.

use super::{
    UiState,
    actions::{Action, ActionMap},
};
use crate::session_glue::AppState;
use egui::{RichText, Ui};

/// Menu trigger labels — `index.html` L15-58 (`data-menu` buttons).
///
/// The File button is the *menu* label, not the first item's action label:
/// reading `Action::NewDocument`'s label here made the slot show `새 문서`
/// (user report 2026-10-18).
const FILE_MENU_LABEL: &str = "파일";

/// Every top-menu entry is a block with the same footprint. Keeping the slot
/// size fixed is what lets a block detach without moving its neighbours.
const BLOCK_SLOT: egui::Vec2 = egui::vec2(88.0, 26.0);
const BLOCK_WINDOW: egui::Vec2 = egui::vec2(264.0, 240.0);
const BLOCK_WINDOW_STEP: f32 = 18.0;

/// The five fixed menubar blocks, in their permanent order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuBlock {
    File,
    Edit,
    View,
    Layer,
    Help,
}

impl MenuBlock {
    const ALL: [Self; 5] = [Self::File, Self::Edit, Self::View, Self::Layer, Self::Help];

    fn index(self) -> usize {
        match self {
            Self::File => 0,
            Self::Edit => 1,
            Self::View => 2,
            Self::Layer => 3,
            Self::Help => 4,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::File => FILE_MENU_LABEL,
            Self::Edit => "편집",
            Self::View => "보기",
            Self::Layer => "레이어",
            Self::Help => "도움말",
        }
    }

    /// Cascade detached windows below the menubar so they do not cover the
    /// block sockets they came from.
    fn default_window_pos(self, ctx: &egui::Context) -> egui::Pos2 {
        let origin = ctx
            .input(|i| i.viewport().inner_rect.map(|r| r.min))
            .unwrap_or(egui::Pos2::ZERO);
        let step = self.index() as f32 * BLOCK_WINDOW_STEP;
        origin + egui::vec2(16.0 + step, 44.0 + step)
    }
}

pub struct Menubar {
    actions: ActionMap,
}

impl Menubar {
    pub fn new() -> Self {
        Self {
            actions: ActionMap::new(),
        }
    }

    /// Render the menubar. Returns any triggered action.
    pub fn show(&mut self, ui: &mut Ui, state: &mut UiState, app: &mut AppState) -> Option<Action> {
        let mut triggered = None;
        // UI-3: `history_buttons.js` disables the top-bar undo/redo while a
        // dialog is open (`disabled = busy || !canUndo`). Read before the
        // closures so no borrow of `state` is live inside them.
        let busy = state.dialogs.active.is_some();
        ui.horizontal(|ui| {
            // Fixed-size sockets: attached blocks draw their menu button here,
            // detached blocks leave a same-sized placeholder. Either way the
            // separator and history buttons stay put.
            for block in MenuBlock::ALL {
                let r = if state.menubar.detached[block.index()] {
                    self.detached_socket(ui, state, block);
                    None
                } else {
                    self.attached_block(ui, state, app, block)
                };
                if triggered.is_none() {
                    triggered = r;
                }
            }
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // `style_juicy.css` J8: the two history actions are first-class
                // buttons — 26px minimum height, 10px side padding, 12px
                // labels — instead of the stock small widgets the top bar had.
                ui.spacing_mut().button_padding = egui::vec2(10.0, 4.0);
                let (can_undo, can_redo) = app.history_state();
                if ui
                    .add_enabled(
                        can_redo && !busy,
                        egui::Button::new(
                            RichText::new(self.actions.label(&Action::Redo)).size(12.0),
                        )
                        .min_size(egui::vec2(0.0, 26.0)),
                    )
                    .on_hover_text("[Ctrl+Y]  다시 적용합니다.")
                    .clicked()
                {
                    triggered = Some(Action::Redo);
                }
                if ui
                    .add_enabled(
                        can_undo && !busy,
                        egui::Button::new(
                            RichText::new(self.actions.label(&Action::Undo)).size(12.0),
                        )
                        .min_size(egui::vec2(0.0, 26.0)),
                    )
                    .on_hover_text("[Ctrl+Z]  되돌립니다.")
                    .clicked()
                {
                    triggered = Some(Action::Undo);
                }
            });
        });

        // Floating blocks live above the normal layout, so moving them around
        // never changes the menubar row or any panel below it.
        for block in MenuBlock::ALL {
            if state.menubar.detached[block.index()]
                && let Some(action) = self.detached_block(ui.ctx(), state, app, block)
                && triggered.is_none()
            {
                triggered = Some(action);
            }
        }
        triggered
    }

    /// One attached block: fixed-size menu button plus its popup. The detach
    /// item is appended to every popup so any block can become a floating
    /// window without changing the bar.
    fn attached_block(
        &mut self,
        ui: &mut Ui,
        state: &mut UiState,
        app: &mut AppState,
        block: MenuBlock,
    ) -> Option<Action> {
        let mut triggered = None;
        let response = block_menu_button(ui, block.label(), |ui| {
            self.block_contents(ui, app, block, &mut triggered);
            block_detach_item(ui, state, block);
        });
        if triggered.is_some() {
            egui::Popup::close_id(ui.ctx(), egui::Popup::default_response_id(&response));
        }
        triggered
    }

    /// Same footprint as an attached block, but empty except for the label.
    /// Clicking the socket puts the block back in the bar.
    fn detached_socket(&mut self, ui: &mut Ui, state: &mut UiState, block: MenuBlock) {
        let response = ui
            .add_sized(
                BLOCK_SLOT,
                egui::Button::new(RichText::new(block.label()).color(egui::Color32::GRAY)),
            )
            .on_hover_text("클릭하여 상단 메뉴에 다시 부착합니다");
        if response.clicked() {
            state.menubar.detached[block.index()] = false;
        }
    }

    /// The detached form of a block: a fixed-size floating window with the
    /// same contents. It lives in an `egui::Area`, so dragging it never
    /// reflows the menubar or the panels below.
    fn detached_block(
        &self,
        ctx: &egui::Context,
        state: &mut UiState,
        app: &mut AppState,
        block: MenuBlock,
    ) -> Option<Action> {
        let mut triggered = None;
        let mut open = true;
        egui::Window::new(block.label())
            .id(egui::Id::new(("pixeldraw.menu_block", block.index())))
            .default_pos(block.default_window_pos(ctx))
            .fixed_size(BLOCK_WINDOW)
            .resizable(false)
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(RichText::new("상단 메뉴 블록").size(11.0).weak());
                ui.separator();
                // Reserve the footer first; otherwise the scroll area would
                // consume the whole fixed-height window and clip the attach
                // button out of view.
                let list_height = (ui.available_height() - 40.0).max(60.0);
                egui::ScrollArea::vertical()
                    .max_height(list_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.block_contents(ui, app, block, &mut triggered);
                    });
                ui.separator();
                if ui.button("상단에 부착").clicked() {
                    state.menubar.detached[block.index()] = false;
                }
            });
        if !open {
            state.menubar.detached[block.index()] = false;
        }
        triggered
    }

    fn block_contents(
        &self,
        ui: &mut Ui,
        app: &mut AppState,
        block: MenuBlock,
        triggered: &mut Option<Action>,
    ) {
        match block {
            MenuBlock::File => self.file_contents(ui, triggered),
            MenuBlock::Edit => self.edit_contents(ui, app, triggered),
            MenuBlock::View => self.view_contents(ui, triggered),
            MenuBlock::Layer => self.layer_contents(ui, triggered),
            MenuBlock::Help => self.help_contents(ui, triggered),
        }
    }
}

impl Default for Menubar {
    fn default() -> Self {
        Self::new()
    }
}

/// A menu button with the fixed block footprint. While its popup is open the
/// menu stays up as long as the pointer is over the slot, the popup, or the
/// gap between them.
fn block_menu_button(ui: &mut Ui, label: &str, contents: impl FnOnce(&mut Ui)) -> egui::Response {
    let response = ui.add_sized(BLOCK_SLOT, egui::Button::new(label));
    let popup_id = egui::Popup::default_response_id(&response);
    let ctx = ui.ctx().clone();
    if egui::Popup::is_id_open(&ctx, popup_id)
        && let Some(pos) = ctx.pointer_hover_pos()
        && !menu_hover_keeps_open(pos, response.rect, ctx.memory(|m| m.area_rect(popup_id)))
    {
        egui::Popup::close_id(&ctx, popup_id);
    }
    egui::Popup::menu(&response).show(contents);
    response
}

/// The "detach" item appended to every attached block's popup.
fn block_detach_item(ui: &mut Ui, state: &mut UiState, block: MenuBlock) {
    ui.separator();
    if ui.button("상단에서 분리").clicked() {
        state.menubar.detached[block.index()] = true;
        ui.close();
    }
}

/// Pointer is still on the button/popup path, so the menu must not close.
fn menu_hover_keeps_open(pos: egui::Pos2, button: egui::Rect, popup: Option<egui::Rect>) -> bool {
    if button.contains(pos) {
        return true;
    }
    let Some(popup) = popup else {
        return false;
    };
    if popup.contains(pos) {
        return true;
    }
    let within_x = pos.x >= button.left() && pos.x <= button.right();
    let between = pos.y >= button.top() && pos.y <= popup.bottom();
    within_x && between
}

/// Integration function for the top-level UI render.
pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, app_state: &mut AppState) {
    let mut menubar = Menubar::new();
    if let Some(action) = menubar.show(ui, ui_state, app_state) {
        let _ = crate::ui::action_router::dispatch(action, app_state, ui_state);
    }
}

mod menus;
#[cfg(test)]
mod tests;
