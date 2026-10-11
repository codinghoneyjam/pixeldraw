//! pixeldraw — native shell entry point.
//!
//! Phase 0.2 stub: the binary only proves the workspace wiring and the engine
//! dependency. Phase 2 replaces `main` with the eframe window and the pixel
//! canvas (docs/native_plan.md §6 Phase 2).

fn main() {
    pixeldraw_app::shell::run();
}
