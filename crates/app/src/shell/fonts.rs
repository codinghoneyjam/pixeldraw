//! Korean font discovery and install.
//!
//! Split out of `shell.rs`: the window owns the frame loop, and the font probe
//! is a separate concern with a separate failure mode (it can fail and the app
//! still runs). Tests for the probe live here rather than in the shell.

use std::path::PathBuf;

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
