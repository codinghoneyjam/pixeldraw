use super::*;
use pixel_engine::view::View;

use crate::input::RawPointer;

/// Headless frame: controller → manager → comp → display pixels.
#[test]
fn headless_stroke_reaches_the_framebuffer() {
    use pixel_engine::display::{DisplayParams, GridMode, draw_display};

    let mut state = AppState::new();
    // The subscription attaches after the boot document, so the queue
    // starts empty; the comp cache still paints everything once.
    state.drain_events();
    let view = View::new(1.0, 0.0, 0.0).expect("valid view");
    let raw = RawPointer::new((10.0, 10.0));
    let accept = |b| b == crate::input::Button::Left;
    let ctx = crate::input::InputCtx {
        view,
        canvas: Some((512, 512)),
        viewport: Some((512.0, 512.0)),
        active_tool_id: "pen",
        tool_accepts_button: &accept,
        tool_cursor: crate::tools::CursorKind::Crosshair,
    };
    let out = state.input.pointer_down(&raw, &ctx);
    assert!(state.input.is_drawing());
    let mut render = false;
    {
        let mut cx = ToolCx {
            session: &mut state.session,
            view,
            need_render: &mut render,
        };
        for call in &out.tool_calls {
            match call {
                crate::input::ToolCall::Down(ev) => {
                    state.tools.pointer_down(ev, &mut cx).expect("dispatch");
                }
                _ => panic!("down frame dispatches Down, got {call:?}"),
            }
        }
    }
    let up = state.input.pointer_up(&raw, &ctx);
    {
        let mut cx = ToolCx {
            session: &mut state.session,
            view,
            need_render: &mut render,
        };
        for call in &up.tool_calls {
            if let crate::input::ToolCall::Up(ev) = call {
                state.tools.pointer_up(ev, &mut cx).expect("dispatch");
            }
        }
    }
    assert!(state.drain_events(), "stroke events request a frame");
    let doc = state.session.doc().expect("doc").clone();
    state.comp.update(&doc, None, frame_budget());
    let params = DisplayParams {
        view,
        canvas_w: doc.canvas.width_px,
        canvas_h: doc.canvas.height_px,
        css_w: 64.0,
        css_h: 64.0,
        dpr: 1.0,
        background: &doc.canvas.background,
        grid: GridMode::Off,
        comp: Some(state.comp.comp()),
    };
    let fb = draw_display(&params);
    assert!(
        fb.rgba.iter().any(|&b| b != 0),
        "the dot reaches the pixels"
    );
    assert!(session_undo_clears(&mut state));
}

fn session_undo_clears(state: &mut AppState) -> bool {
    use pixel_engine::display::{DisplayParams, GridMode, draw_display};
    use pixel_engine::view::View;

    assert!(state.session.undo().expect("undo"));
    assert!(state.drain_events());
    let doc = state.session.doc().expect("doc").clone();
    // Paint what remains dirty, then confirm a flat white frame.
    let mut guard = 0;
    while state.comp.update(&doc, None, frame_budget()) {
        guard += 1;
        assert!(guard < 8, "the cache settles");
    }
    let view = View::new(1.0, 0.0, 0.0).expect("valid view");
    let params = DisplayParams {
        view,
        canvas_w: doc.canvas.width_px,
        canvas_h: doc.canvas.height_px,
        css_w: 64.0,
        css_h: 64.0,
        dpr: 1.0,
        background: "#ffffff",
        grid: GridMode::Off,
        comp: Some(state.comp.comp()),
    };
    let fb = draw_display(&params);
    fb.rgba
        .as_chunks::<4>()
        .0
        .iter()
        .all(|px| *px == [255, 255, 255, 255])
}

/// Phase 3.9 boot order: the settings restore runs before the first
/// document and before the listener attaches (`app.js` L83-99), so nothing
/// is queued and the file cannot be written back by its own events.
#[test]
fn boot_applies_settings_without_queueing_events() {
    let path = std::env::temp_dir().join(format!(
        "pixeldraw-appstate-settings-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, r##"{"penSize": 12, "activeTool": "eraser"}"##).expect("seed");
    let store = SettingsStore::load_from(Some(path.clone()));
    let mut state = AppState::boot_with(store, AutosaveGlue::disabled());
    assert_eq!(state.session.settings().pen_size, 12);
    assert_eq!(state.session.settings().active_tool, "eraser");
    assert_eq!(
        state.tools.active_id(),
        "eraser",
        "the manager follows the restored activeTool"
    );
    assert!(state.queue.borrow().is_empty(), "restore is listener-free");
    assert!(!state.drain_events());
    let _ = std::fs::remove_file(&path);
}

/// Phase 3.10b: restoring installs the autosave document through the queue
/// and marks the dialog resolved. The restored document reads **clean** —
/// the JS oracle returns `isDirty() == false` after
/// `loadDocument(doc, {markSaved:false})` (checked with node on
/// 2026-10-17; the roadmap's "복원 직후 dirty ●" note is wrong).
#[test]
fn restoring_an_autosave_installs_the_document_clean() {
    let path = std::env::temp_dir().join(format!(
        "pixeldraw-appstate-autosave-{}.json",
        std::process::id()
    ));
    let seed = AppState::new();
    let doc = seed.session.doc().expect("doc");
    let doc_id = doc.id.clone();
    let bytes = crate::ui::autosave::AutosaveRecord::build(doc, 42)
        .expect("build")
        .encode();
    std::fs::write(&path, &bytes).expect("seed");

    let glue = AutosaveGlue::boot_from(Some(path.clone()));
    assert!(glue.peek().is_some(), "the record is offered");
    let mut state = AppState::boot_with(SettingsStore::empty(), glue);
    assert!(state.autosave_peek().is_some());
    assert!(state.restore_autosave());
    assert_eq!(state.session.doc().expect("doc").id, doc_id);
    assert!(
        !state.session.history().is_dirty(),
        "matches the JS oracle: loadDocument({{markSaved:false}}) is clean"
    );
    assert!(state.autosave_peek().is_none(), "the dialog is resolved");
    let _ = std::fs::remove_file(&path);
}
