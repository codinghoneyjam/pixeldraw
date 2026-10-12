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

/// `src-tauri/tauri.conf.json` window geometry, mirrored here.
pub const WINDOW_TITLE: &str = "Pixeldraw";
pub const WINDOW_SIZE: [f32; 2] = [1280.0, 820.0];
pub const WINDOW_MIN_SIZE: [f32; 2] = [900.0, 560.0];

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
}

impl PixeldrawApp {
    pub fn new(ctx: &egui::Context) -> Self {
        if !install_korean_font(ctx) {
            eprintln!("pixeldraw: no Korean system font found; Latin fallback only");
        }
        Self {
            state: AppState::new(),
            ui_state: UiState::default(),
            texture: CanvasTexture::new(),
            fb: Framebuffer::new(1, 1),
        }
    }

    fn say(&mut self, text: impl Into<String>) {
        crate::ui::shared::toasts::info(&mut self.ui_state, text);
    }
}

impl eframe::App for PixeldrawApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx();

        // Handle keyboard shortcuts
        if let Some(action) = self.ui_state.menubar.actions.find_matching(ctx) {
            self.dispatch_action(action);
        }

        // Render all UI panels using the root ui
        render_ui(ui, &mut self.ui_state, &mut self.state);

        // Central canvas area
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                show_canvas(ui, &mut self.state, &mut self.texture, &mut self.fb);
            });
    }
}

impl PixeldrawApp {
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
                        let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
                        self.say("저장됨");
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
                    "unit" => "pixel",
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
            ToolPen | ToolEraser | ToolLine | ToolRect | ToolEllipse | ToolFill
            | ToolEyedropper | ToolHand | ToolText | ToolGradient | ToolShape => {
                let tool_id = match action {
                    ToolPen => "pen",
                    ToolEraser => "eraser",
                    ToolLine => "line",
                    ToolRect => "rect",
                    ToolEllipse => "ellipse",
                    ToolFill => "fill",
                    ToolEyedropper => "eyedropper",
                    ToolHand => "hand",
                    ToolText => "text",
                    ToolGradient => "gradient",
                    ToolShape => "shape",
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
    fn window_geometry_mirrors_tauri_conf() {
        assert_eq!(WINDOW_TITLE, "Pixeldraw");
        assert_eq!(WINDOW_SIZE, [1280.0, 820.0]);
        assert_eq!(WINDOW_MIN_SIZE, [900.0, 560.0]);
    }
}
