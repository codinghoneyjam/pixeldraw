//! Native shell — eframe window, native UI, canvas wiring.
//!
//! Phase 3: Full native UI with menubar, toolbar, optionbar, statusbar,
//! dialogs, and shared components (toast, tooltip, collapsible).

use std::path::PathBuf;

use pixel_engine::display::Framebuffer;

use crate::canvas_texture::CanvasTexture;
use crate::canvas_view::show_canvas;
use crate::file_ops::{export_png_bytes, open_json_bytes, open_png_bytes, save_json_bytes};
use crate::session_glue::AppState;
use crate::ui::{UiState, render_ui};

/// Window geometry. The title and the minimum still mirror
/// `src-tauri/tauri.conf.json` (1280×820), but the port opens at the user's
/// 16:9 default of 1920×1080 and shrinks it — same ratio, never bigger — on a
/// monitor that cannot hold it (`fit_window_to_monitor`).
pub const WINDOW_TITLE: &str = "Pixeldraw";
pub const WINDOW_SIZE: [f32; 2] = [1920.0, 1080.0];
pub const WINDOW_MIN_SIZE: [f32; 2] = [900.0, 560.0];
/// Room the fitted window must leave: winit reports the whole monitor, not the
/// work area, so the taskbar strip comes off the bottom and a symmetric side
/// margin frames the rest. The title bar is not guessed — it comes from the
/// live window (`outer - inner` decorations), since it counts against the fit.
const WINDOW_BOTTOM_MARGIN: f32 = 48.0;
const WINDOW_SIDE_MARGIN: f32 = 16.0;

/// Keep the 16:9 startup target inside `monitor` (egui points): one uniform
/// scale, never above 1.0, so the ratio survives a small monitor. `None` means
/// the target already fits, and the window stays where eframe centred it —
/// nothing moves on a monitor that can hold the target.
/// `outer`/`inner` are the current window rects; they supply the decoration
/// size, which does not scale with the client area.
fn fit_window_to_monitor(
    target: egui::Vec2,
    monitor: egui::Vec2,
    outer: egui::Rect,
    inner: egui::Vec2,
) -> Option<(egui::Vec2, egui::Pos2)> {
    let decorations = (outer.size() - inner).max(egui::Vec2::ZERO);
    let work = monitor - egui::Vec2::new(0.0, WINDOW_BOTTOM_MARGIN);
    // The box the outer rect has to fit into: the work area minus the side
    // margin, so the window lands with a symmetric gap all round.
    let area = work - egui::Vec2::splat(2.0 * WINDOW_SIDE_MARGIN);
    let avail = area - decorations;
    let scale = (avail.x / target.x).min(avail.y / target.y).min(1.0);
    if !scale.is_finite() || scale >= 1.0 {
        return None;
    }
    // Floor, not round: rounding up could eat the last fraction of the margin.
    let size = (target * scale).floor();
    // Decorations do not change with the client area, so the outer box grows
    // by exactly the inner delta.
    let outer_size = outer.size() + (size - inner);
    let pos = egui::pos2(
        WINDOW_SIDE_MARGIN + (area.x - outer_size.x) / 2.0,
        WINDOW_SIDE_MARGIN + (area.y - outer_size.y) / 2.0,
    );
    Some((size, pos))
}

/// System Korean fonts, first hit wins. Bundling is undecided (license +
/// offline-exe size) — see the W7 log note — so the system copy is primary.
pub fn system_font_candidates() -> Vec<PathBuf> {
    const NAMES: [&str; 3] = ["malgun.ttf", "malgunbd.ttf", "NanumGothic.ttf"];
    let mut out = Vec::with_capacity(NAMES.len() + 1);
    if let Ok(windir) = std::env::var("SystemRoot") {
        for name in NAMES {
            out.push(PathBuf::from(&windir).join("Fonts").join(name));
        }
    }
    out.push(PathBuf::from(r"C:\Windows\Fonts\malgun.ttf"));
    out
}

/// First path that exists and reads as a file.
pub fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.is_file()).cloned()
}

/// Install the Korean fallback into first position of Proportional/Mono.
/// Returns whether a font was installed; without one the UI falls back to
/// egui's built-in Latin set and logs a warning in `run`.
pub fn install_korean_font(ctx: &egui::Context) -> bool {
    let Some(path) = first_existing(&system_font_candidates()) else {
        return false;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return false;
    };
    let name = "korean-fallback".to_string();
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(name.clone(), egui::FontData::from_owned(bytes).into());
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            list.insert(0, name.clone());
        }
    }
    ctx.set_fonts(fonts);
    true
}

pub struct PixeldrawApp {
    state: AppState,
    ui_state: UiState,
    texture: CanvasTexture,
    fb: Framebuffer,
    /// The close guard let the window go (3.10b `beforeunload` parity).
    allow_close: bool,
    /// The one-shot startup size fit already ran (a manual resize after it is
    /// never snapped back).
    window_fitted: bool,
}

impl PixeldrawApp {
    pub fn new(ctx: &egui::Context) -> Self {
        if !install_korean_font(ctx) {
            eprintln!("pixeldraw: no Korean system font found; Latin fallback only");
        }
        ctx.set_visuals(crate::ui::theme::neon_dark());
        ctx.style_mut_of(egui::Theme::Dark, crate::ui::theme::apply_spacing);
        let mut ui_state = UiState::default();
        // Restore the collapsed-panel map written by the section headers. Read
        // once at boot; the file is only written again when a header is clicked.
        ui_state.panels.collapsed = pixel_platform::load_collapsed_panels()
            .into_iter()
            .filter(|(_, collapsed)| *collapsed)
            .map(|(id, _)| id)
            .collect();
        // Boot order (`ui/app.js` L83-99): settings + recent colors, then the
        // autosave peek and its restore dialog. `AppState::boot` restores the
        // settings before the first document and before any listener attaches.
        let state = AppState::boot();
        ui_state.panels.recent = state.settings_store.recent().to_vec();
        if let Some((name, when)) = state.autosave_peek() {
            ui_state.dialogs.active =
                Some(crate::ui::dialogs::Dialog::restore_autosave(name, when));
        }
        // The option bar dispatches on the active tool, so it has to start
        // somewhere: without this the strip is empty until the first click.
        ui_state.toolbar.active_tool = state.session.settings().active_tool.to_string();
        Self {
            state,
            ui_state,
            texture: CanvasTexture::new(),
            fb: Framebuffer::new(1, 1),
            allow_close: false,
            window_fitted: false,
        }
    }

    /// Startup window fit, one shot: the window is created at the 16:9 target
    /// and the first frame that knows the monitor shrinks it when the target
    /// does not fit. The latch is what keeps a later manual resize intact.
    fn fit_startup_window(&mut self, ctx: &egui::Context) {
        if self.window_fitted {
            return;
        }
        let (monitor, outer, inner) = ctx.input(|i| {
            let viewport = i.viewport();
            (
                viewport.monitor_size,
                viewport.outer_rect,
                viewport.inner_rect.map(|rect| rect.size()),
            )
        });
        let (Some(monitor), Some(outer), Some(inner)) = (monitor, outer, inner) else {
            return; // the integration has not reported the window yet
        };
        self.window_fitted = true;
        let target = egui::Vec2::new(WINDOW_SIZE[0], WINDOW_SIZE[1]);
        if let Some((size, pos)) = fit_window_to_monitor(target, monitor, outer, inner) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }
    }

    fn say(&mut self, text: impl Into<String>) {
        crate::ui::shared::toasts::info(&mut self.ui_state, text);
    }
}

impl eframe::App for PixeldrawApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // An owned clone (cheap Arc) so the `ui` borrow can be released while
        // `ctx` is still used across `render_ui`.
        let ctx = ui.ctx().clone();

        // Startup geometry: shrink the 16:9 window to the monitor if needed
        // (once — afterwards the user owns the size).
        self.fit_startup_window(&ctx);

        // Phase 3.10b close guard: flush the pending autosave ("pagehide"
        // parity), then keep unsaved changes behind a confirm dialog
        // (`beforeunload` parity; the JS relied on the browser prompt).
        if ctx.input(|i| i.viewport().close_requested()) {
            self.state.flush_autosave();
            if self.state.session.history().is_dirty() && !self.allow_close {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.ui_state.dialogs.active = Some(crate::ui::dialogs::Dialog::confirm_exit());
            } else {
                self.allow_close = true;
            }
        } else if !ctx.input(|i| i.focused) {
            // `visibilitychange` parity: best effort while the window is
            // unfocused (a no-op unless something is dirty).
            self.state.flush_autosave();
        }

        // UI-3 shortcut gates (`input.js` keydown order): an editable target
        // consumes only Escape, flipping it into a blur; a busy dialog ignores
        // everything. The canvas key stream reads the same `ui_busy`.
        let busy = self.ui_state.dialogs.active.is_some();
        self.state.ui_busy = busy;
        if ctx.egui_wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape))
                && let Some(id) = ctx.memory(|m| m.focused())
            {
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
        } else if let Some(action) = self
            .ui_state
            .menubar
            .actions
            .find_matching_guarded(&ctx, busy)
        {
            self.dispatch_action(action);
        }

        // Render all UI panels using the root ui
        render_ui(ui, &mut self.ui_state, &mut self.state);
        self.handle_dialog_results(&ctx);

        // Central canvas area
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                show_canvas(
                    ui,
                    &mut self.state,
                    &mut self.texture,
                    &mut self.fb,
                    &mut self.ui_state.statusbar.cursor_pos,
                );
            });

        // Phase 3.9/3.10 frame persistence: recent-colors snapshot, the 250 ms
        // settings debounce, and the 800 ms autosave schedule.
        let now = std::time::Instant::now();
        self.state
            .settings_store
            .note_recent(&self.ui_state.panels.recent, now);
        self.state.tick_persistence(now);
    }
}

impl PixeldrawApp {
    /// Dialog outcomes the shell owns: the boot autosave prompt (3.10b) and
    /// the close guard. The older dialog kinds keep their existing post-close
    /// path inside `ui/dialogs.rs`.
    fn handle_dialog_results(&mut self, ctx: &egui::Context) {
        use crate::ui::dialogs::DialogResult;
        let Some(result) = self.ui_state.dialogs.pending_result.take() else {
            return;
        };
        match result {
            DialogResult::AutosaveRestore(true) => {
                self.state.restore_autosave();
            }
            DialogResult::AutosaveRestore(false) => {
                self.state.discard_autosave();
            }
            DialogResult::ConfirmExit(true) => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            DialogResult::ConfirmExit(false) => {}
            _ => {}
        }
    }

    fn dispatch_action(&mut self, action: crate::ui::actions::Action) {
        use crate::ui::actions::Action::*;
        match action {
            NewDocument => {
                self.ui_state.dialogs.active = Some(crate::ui::dialogs::Dialog::new_document());
            }
            OpenFile => {
                if let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Any) {
                    self.open_any(&path);
                }
            }
            Save => {
                if let Ok(bytes) = save_json_bytes(&self.state) {
                    if let Some(path) =
                        pixel_platform::pick_save_file(pixel_platform::FileKind::Json, "untitled")
                    {
                        match pixel_platform::write_bytes_atomic(&path, &bytes) {
                            Ok(()) => {
                                // JS `doSave`: `history.markSaved()` only on a
                                // successful write — the statusbar dirty dot
                                // and the close guard read this.
                                self.state.session.history_mut().mark_saved();
                                self.say("저장됨");
                            }
                            Err(e) => self.say(format!("저장 실패: {e}")),
                        }
                    }
                } else {
                    self.say("저장 실패");
                }
            }
            SaveAs => {
                // Same as Save for now
            }
            ExportPng => {
                if let Ok(bytes) = export_png_bytes(&self.state) {
                    if let Some(path) =
                        pixel_platform::pick_save_file(pixel_platform::FileKind::Png, "canvas")
                    {
                        let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
                        self.say("PNG 저장됨");
                    }
                } else {
                    self.say("PNG 실패");
                }
            }
            Undo => {
                let _ = self.state.session.undo();
            }
            Redo => {
                let _ = self.state.session.redo();
            }
            ZoomIn => {
                self.state.view_store.zoom_in();
            }
            ZoomOut => {
                self.state.view_store.zoom_out();
            }
            ZoomFit => {
                self.state.view_store.fit();
            }
            ZoomActual => {
                self.state.view_store.actual_size();
            }
            ToggleGrid => {
                let current = self.state.session.settings().grid_mode;
                let next = match current {
                    "off" => "unit",
                    "unit" => "tile",
                    "tile" => "pixel",
                    _ => "off",
                };
                let _ = self
                    .state
                    .session
                    .set_setting("gridMode", pixel_engine::model::SettingInput::Str(next));
            }
            NewLayer => {
                let _ = self.state.session.add_layer(None);
            }
            DuplicateLayer => {
                if let Some(id) = self.state.session.doc().map(|d| d.active_layer_id.clone()) {
                    let _ = self.state.session.duplicate_layer(&id);
                }
            }
            DeleteLayer => {
                if let Some(id) = self.state.session.doc().map(|d| d.active_layer_id.clone()) {
                    let _ = self.state.session.remove_layer(&id);
                }
            }
            MergeDown => {
                if let Some(id) = self.state.session.doc().map(|d| d.active_layer_id.clone()) {
                    let _ = self.state.session.merge_down(&id);
                }
            }
            Preferences => {
                self.ui_state.dialogs.active = Some(crate::ui::dialogs::Dialog::preferences());
            }
            ShowHelp => {
                self.ui_state.dialogs.active = Some(crate::ui::dialogs::Dialog::new(
                    crate::ui::dialogs::DialogKind::Help,
                ));
            }
            ToolPen | ToolEraser | ToolFill | ToolEyedropper | ToolLine | ToolRect | ToolRrect
            | ToolEllipse | ToolPolygon | ToolArc | ToolChord | ToolRadialGradient | ToolText
            | ToolHand => {
                let tool_id = match action {
                    ToolPen => "pen",
                    ToolEraser => "eraser",
                    ToolFill => "fill",
                    ToolEyedropper => "eyedropper",
                    ToolLine => "line",
                    ToolRect => "rect",
                    ToolRrect => "rrect",
                    ToolEllipse => "ellipse",
                    ToolPolygon => "polygon",
                    ToolArc => "arc",
                    ToolChord => "chord",
                    ToolRadialGradient => "radial_gradient",
                    ToolText => "text",
                    ToolHand => "hand",
                    _ => unreachable!(),
                };
                self.ui_state.toolbar.active_tool = tool_id.to_string();
                let _ = self.state.session.set_setting(
                    "activeTool",
                    pixel_engine::model::SettingInput::Str(tool_id),
                );
            }
            _ => {}
        }
    }

    fn open_any(&mut self, path: &std::path::Path) {
        let bytes = match pixel_platform::read_bytes(path) {
            Ok(b) => b,
            Err(e) => {
                self.say(format!("읽기 실패: {e}"));
                return;
            }
        };
        let is_png = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
        let result = if is_png {
            open_png_bytes(&mut self.state, &bytes)
        } else {
            open_json_bytes(&mut self.state, &bytes)
        };
        match result {
            Ok(()) => self.say("열기 완료"),
            Err(e) => self.say(format!("열기 실패: {e}")),
        }
    }
}

/// Open the native window. Blocks until exit; reports setup failures on stderr.
pub fn run() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size(WINDOW_SIZE)
            .with_min_inner_size(WINDOW_MIN_SIZE),
        // The 16:9 target is centred on the primary monitor; the first frame
        // re-centres it when it had to shrink.
        centered: true,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|cc| Ok(Box::new(PixeldrawApp::new(&cc.egui_ctx)) as Box<dyn eframe::App>)),
    ) {
        eprintln!("pixeldraw: failed to open the native window: {e}");
    }
}

pub fn open_any(state: &mut AppState, path: &std::path::Path) {
    let bytes = match pixel_platform::read_bytes(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pixeldraw: 읽기 실패: {e}");
            return;
        }
    };
    let is_png = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
    let result = if is_png {
        open_png_bytes(state, &bytes)
    } else {
        open_json_bytes(state, &bytes)
    };
    match result {
        Ok(()) => {}
        Err(e) => eprintln!("pixeldraw: 열기 실패: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_existing_picks_readable_files() {
        let dir = std::env::temp_dir().join("pixeldraw-font-probe");
        let _ = std::fs::create_dir_all(&dir);
        let missing = dir.join("no-such-font.ttf");
        let present = dir.join("present.ttf");
        std::fs::write(&present, [0u8; 8]).expect("probe file");
        assert_eq!(
            first_existing(&[missing.clone(), present.clone()]),
            Some(present)
        );
        assert_eq!(first_existing(&[missing]), None);
        let _ = std::fs::remove_file(dir.join("present.ttf"));
    }

    #[test]
    fn window_defaults_are_a_16x9_target() {
        assert_eq!(WINDOW_TITLE, "Pixeldraw");
        assert_eq!(WINDOW_SIZE, [1920.0, 1080.0]);
        // 16:9, the ratio the JS `#dt-app` viewport assumes.
        assert_eq!(WINDOW_SIZE[0] / WINDOW_SIZE[1], 16.0 / 9.0);
        assert_eq!(WINDOW_MIN_SIZE, [900.0, 560.0]);
    }

    /// The startup fit only ever shrinks, always uniformly, and keeps the
    /// window centred while it does.
    #[test]
    fn the_startup_window_only_shrinks_to_fit() {
        let target = egui::Vec2::new(WINDOW_SIZE[0], WINDOW_SIZE[1]);
        let outer = egui::Rect::from_min_size(egui::pos2(300.0, 150.0), egui::vec2(1930.0, 1120.0));
        let inner = egui::vec2(1920.0, 1080.0);

        // A monitor the target fits in (the dev box's 2560×1440 points): the
        // window is left exactly where eframe centred it.
        assert_eq!(
            fit_window_to_monitor(target, egui::vec2(2560.0, 1440.0), outer, inner),
            None
        );

        // A 1920×1080 desktop: the height is what gives, the ratio stays, and
        // the outer box (title bar + taskbar margin) stays inside the work area.
        let (size, pos) = fit_window_to_monitor(target, egui::vec2(1920.0, 1080.0), outer, inner)
            .expect("a 1080p monitor needs a fit");
        let deco = outer.size() - inner;
        let work = egui::vec2(1920.0, 1080.0 - WINDOW_BOTTOM_MARGIN);
        let new_outer = deco + size;
        assert!((size.x / size.y - 16.0 / 9.0).abs() < 0.002);
        assert!(pos.x >= WINDOW_SIDE_MARGIN - 0.001);
        assert!(pos.x + new_outer.x <= work.x - WINDOW_SIDE_MARGIN + 0.001);
        assert!(pos.y + new_outer.y <= work.y - WINDOW_SIDE_MARGIN + 0.001);
        // Centred on the work area, so the taskbar strip stays clear.
        assert!((pos.x - (work.x - new_outer.x) / 2.0).abs() < 0.001);

        // A laptop panel: a smaller uniform scale, still 16:9 and inside.
        let (small, small_pos) =
            fit_window_to_monitor(target, egui::vec2(1366.0, 768.0), outer, inner).expect("fit");
        let small_outer = outer.size() + (small - inner);
        assert!(small_pos.x + small_outer.x <= 1366.0 - WINDOW_SIDE_MARGIN + 0.001);
        assert!(
            small_pos.y + small_outer.y
                <= 768.0 - WINDOW_BOTTOM_MARGIN - WINDOW_SIDE_MARGIN + 0.001
        );
        assert!((small.x / small.y - 16.0 / 9.0).abs() < 0.002);
        assert!(small.x < size.x && small.y < size.y);
    }
}
