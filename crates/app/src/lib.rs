//! pixeldraw_app ??native UI shell entry point.
//
// This library is the phase-2 entry used by the native app crate.
// The public surface exposes the shell's run() function and internal modules
// for testing.

pub mod canvas_texture;
pub mod canvas_view;
pub mod file_ops;
pub mod input;
pub mod input_adapter;
pub mod overlay_raster;
pub mod session_glue;
pub mod shell;
pub mod tools;

pub use shell::run;

#[cfg(test)]
mod tests {
    #[test]
    fn app_lib_exports_tools_and_input() {
        let _ = super::input::PointerKind::Mouse;
        let _ = super::tools::CursorKind::Crosshair;
    }
}
