//! Native shell — eframe window, temp toolbar, canvas wiring.
//!
//! Phase 2.8 bones: the window opens at the Tauri geometry
//! (`src-tauri/tauri.conf.json`: 1280×820, min 900×560), the top bar holds the
//! minimum clickable set (tools, undo/redo, file, zoom), and the centre is the
//! pixel canvas. The bar is temporary — Phase 3 replaces it with the menu and
//! panels — which is why it lives in one function, not a module.

use std::path::PathBuf;

use pixel_engine::display::Framebuffer;

use crate::canvas_texture::CanvasTexture;
use crate::canvas_view::show_canvas;
use crate::file_ops::{
    export_png_bytes, new_document, open_json_bytes, open_png_bytes, save_json_bytes,
};
use crate::session_glue::AppState;

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
    texture: CanvasTexture,
    fb: Framebuffer,
    notice: Option<String>,
}

impl PixeldrawApp {
    pub fn new(ctx: &egui::Context) -> Self {
        if !install_korean_font(ctx) {
            eprintln!("pixeldraw: no Korean system font found; Latin fallback only");
        }
        Self {
            state: AppState::new(),
            texture: CanvasTexture::new(),
            fb: Framebuffer::new(1, 1),
            notice: None,
        }
    }

    fn say(&mut self, text: impl Into<String>) {
        self.notice = Some(text.into());
    }

    /// Temporary toolbar — file, history, tools, zoom. Phase 3 replaces all of
    /// this with the menu, panels and dialogs.
    fn temp_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let tools = [
                ("연필", "pen"),
                ("지우개", "eraser"),
                ("직선", "line"),
                ("사각형", "rect"),
                ("타원", "ellipse"),
                ("페인트통", "fill"),
                ("손", "hand"),
            ];
            for (label, id) in tools {
                let active = self.state.session.settings().active_tool == id;
                if ui.selectable_label(active, label).clicked() {
                    let _ = self
                        .state
                        .session
                        .set_setting("activeTool", pixel_engine::model::SettingInput::Str(id));
                }
            }
            ui.separator();
            let (can_undo, can_redo) = self.state.history_state();
            if ui.add_enabled(can_undo, egui::Button::new("↩")).clicked() {
                let _ = self.state.session.undo();
            }
            if ui.add_enabled(can_redo, egui::Button::new("↪")).clicked() {
                let _ = self.state.session.redo();
            }
            ui.separator();
            if ui.button("새 문서").clicked() {
                match new_document(&mut self.state, 512, 512) {
                    Ok(()) => self.say("새 문서"),
                    Err(e) => self.say(format!("새 문서 실패: {e}")),
                }
            }
            if ui.button("열기").clicked()
                && let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Any)
            {
                self.open_any(&path);
            }
            if ui.button("저장").clicked() {
                match save_json_bytes(&self.state) {
                    Ok(bytes) => {
                        if let Some(path) = pixel_platform::pick_save_file(
                            pixel_platform::FileKind::Json,
                            "untitled",
                        ) {
                            match pixel_platform::write_bytes_atomic(&path, &bytes) {
                                Ok(()) => self.say("저장됨"),
                                Err(e) => self.say(format!("저장 실패: {e}")),
                            }
                        }
                    }
                    Err(e) => self.say(format!("저장 실패: {e}")),
                }
            }
            if ui.button("PNG").clicked() {
                match export_png_bytes(&self.state) {
                    Ok(bytes) => {
                        if let Some(path) =
                            pixel_platform::pick_save_file(pixel_platform::FileKind::Png, "canvas")
                        {
                            match pixel_platform::write_bytes_atomic(&path, &bytes) {
                                Ok(()) => self.say("PNG 저장됨"),
                                Err(e) => self.say(format!("PNG 실패: {e}")),
                            }
                        }
                    }
                    Err(e) => self.say(format!("PNG 실패: {e}")),
                }
            }
            ui.separator();
            if ui.button("−").clicked() {
                self.state.view_store.zoom_out();
            }
            if ui.button("+").clicked() {
                self.state.view_store.zoom_in();
            }
            if ui.button("맞춤").clicked() {
                self.state.view_store.fit();
            }
            if ui.button("1:1").clicked() {
                self.state.view_store.actual_size();
            }
            if let Some(notice) = self.notice.clone() {
                ui.separator();
                ui.label(notice);
            } else if let Some(last) = self.state.status.last() {
                ui.separator();
                ui.label(&last.text);
            }
        });
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

impl eframe::App for PixeldrawApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // No panels in egui 0.36: toolbar first, canvas takes the rest.
        self.temp_toolbar(ui);
        ui.separator();
        show_canvas(ui, &mut self.state, &mut self.texture, &mut self.fb);
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
