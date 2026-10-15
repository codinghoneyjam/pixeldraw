//! Canvas frame — viewport, input, tools, comp, display, overlay, texture.
//!
//! One `show_canvas` call is one frame: read the host input, dispatch it
//! through the controller into the tools, drain the session queue, repaint
//! dirty chunks within the frame budget, raster the display framebuffer plus
//! the active tool's overlay, and upload the texture. The temporary toolbar
//! lives in `shell`; this module owns the canvas rectangle.

use pixel_engine::display::{DisplayParams, draw_display_into};
use pixel_engine::display::{Framebuffer, GridMode};

use crate::canvas_texture::CanvasTexture;
use crate::input::{InputCtx, ToolCall};
use crate::input_adapter::{FrameInput, poll_frame_input, wheel_mode};
use crate::overlay_raster::rasterize_overlay;
use crate::session_glue::{AppState, frame_budget};
use crate::tools::api::{CursorKind, OverlayInfo, ToolCx};

/// Translate one egui frame of input into controller outcomes and tool calls.
fn dispatch_frame_input(state: &mut AppState, fi: &FrameInput) -> bool {
    let mut render = false;
    let canvas = state
        .session
        .doc()
        .map(|d| (d.canvas.width_px, d.canvas.height_px));
    let vp = state.view_store.viewport();
    let viewport = Some((vp.width, vp.height));
    let view = state.view_store.view();
    let active_id = state.session.settings().active_tool;
    let accepts = |b: crate::input::Button| {
        state
            .tools
            .get(active_id)
            .is_some_and(|t| t.accepts_button(b))
    };
    let cursor = state.tools.cursor();
    let ctx = InputCtx {
        view,
        canvas,
        viewport,
        active_tool_id: active_id,
        tool_accepts_button: &accepts,
        tool_cursor: cursor,
    };
    let mut outcomes = vec![];
    for raw in &fi.down {
        outcomes.push(state.input.pointer_down(raw, &ctx));
    }
    // One move per frame, only when the pointer actually travelled.
    for raw in &fi.moves {
        let travelled = state.last_pointer.is_some_and(|(lx, ly)| {
            (raw.pos.0 - lx).abs() > f64::EPSILON || (raw.pos.1 - ly).abs() > f64::EPSILON
        }) || state.last_pointer.is_none();
        if travelled {
            outcomes.push(state.input.pointer_move(raw, &ctx));
        }
        state.last_pointer = Some(raw.pos);
    }
    for raw in &fi.up {
        outcomes.push(state.input.pointer_up(raw, &ctx));
        state.last_pointer = Some(raw.pos);
    }
    if let Some(wheel) = &fi.wheel {
        outcomes.push(
            state
                .input
                .wheel(wheel.delta_y, wheel_mode(), wheel.shift, wheel.pos, &ctx),
        );
    }
    for (key, is_down) in &fi.keys {
        if *is_down && !fi.text_focused {
            outcomes.push(state.input.key_down(key, &ctx));
        } else if !*is_down {
            outcomes.push(state.input.key_up(key, &ctx));
        }
    }
    for outcome in outcomes {
        render |= outcome.need_render;
        if let Some(view) = outcome.new_view {
            state.view_store.set(view);
            render = true;
        }
        let view = state.view_store.view();
        let mut cx = ToolCx {
            session: &mut state.session,
            view,
            need_render: &mut render,
        };
        for call in &outcome.tool_calls {
            let r = match call {
                ToolCall::Down(ev) => state.tools.pointer_down(ev, &mut cx),
                ToolCall::Move(ev) => state.tools.pointer_move(ev, &mut cx),
                ToolCall::Up(ev) => state.tools.pointer_up(ev, &mut cx),
                ToolCall::Cancel => state.tools.cancel(&mut cx),
                ToolCall::Hover(ev) => state.tools.hover(ev.as_ref(), &mut cx),
            };
            if r.is_err() {
                break;
            }
        }
        let cursor = state.tools.cursor();
        let _ = cursor;
    }
    // Re-resolve the cursor for the host after the dust settles.
    render
}

/// Paint one canvas frame into `fb` and upload it. Returns repaint demand.
pub fn render_frame(
    state: &mut AppState,
    fb: &mut Framebuffer,
    css_w: f64,
    css_h: f64,
    dpr: f64,
) -> bool {
    let mut render = state.drain_events();
    let visible = state.view_store.visible_chunk_range();
    // **The document is borrowed, not cloned.**
    // every layer, every chunk — **once per frame**. `docs/perf_js_vs_rust.md`
    let AppState { session, comp, .. } = state;
    let Some(doc) = session.doc() else {
        return render;
    };
    if comp.update(doc, visible, frame_budget()) {
        render = true;
    }
    let s = session.settings();
    let params = DisplayParams {
        view: state.view_store.view(),
        canvas_w: doc.canvas.width_px,
        canvas_h: doc.canvas.height_px,
        css_w,
        css_h,
        dpr,
        background: &doc.canvas.background,
        grid: GridMode::from(s.grid_mode),
        comp: Some(comp.comp()),
    };
    draw_display_into(&params, fb);
    let mut ops = vec![];
    state.tools.overlay(
        &OverlayInfo {
            view: state.view_store.view(),
            dpr,
        },
        &mut ops,
    );
    rasterize_overlay(fb, &ops, state.view_store.view(), dpr);
    render || state.comp.dirty_len() > 0
}

fn cursor_icon(cursor: CursorKind) -> egui::CursorIcon {
    match cursor {
        CursorKind::Crosshair
        | CursorKind::Pen
        | CursorKind::Eraser
        | CursorKind::Fill
        | CursorKind::Eyedropper => egui::CursorIcon::Crosshair,
        CursorKind::Grab => egui::CursorIcon::Grab,
        CursorKind::Grabbing => egui::CursorIcon::Grabbing,
        CursorKind::Cell => egui::CursorIcon::Cell,
        CursorKind::Copy => egui::CursorIcon::Copy,
        CursorKind::Text => egui::CursorIcon::Text,
        CursorKind::Default => egui::CursorIcon::Default,
    }
}

/// The canvas widget: input → frame → texture → paint.
pub fn show_canvas(
    ui: &mut egui::Ui,
    state: &mut AppState,
    texture: &mut CanvasTexture,
    fb: &mut Framebuffer,
) {
    let avail = ui.available_rect_before_wrap();
    state
        .view_store
        .set_viewport(avail.width() as f64, avail.height() as f64);
    let fi = poll_frame_input(ui.ctx(), avail.min);
    let input_render = dispatch_frame_input(state, &fi);
    let ppp = ui.pixels_per_point();
    let frame_render = render_frame(
        state,
        fb,
        avail.width() as f64,
        avail.height() as f64,
        ppp as f64,
    );
    texture.upload(ui.ctx(), &fb.rgba, fb.width as usize, fb.height as usize);
    // 1:1 paint at the widget origin.
    let rect = egui::Rect::from_min_size(
        avail.min,
        egui::vec2(fb.width as f32 / ppp, fb.height as f32 / ppp),
    );
    let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
    texture.paint(ui, response.rect);
    ui.ctx().set_cursor_icon(cursor_icon(state.tools.cursor()));
    if input_render || frame_render || state.input.is_drawing() {
        ui.ctx().request_repaint();
    }
    // Hover preview while idle: a still frame never repaints otherwise.
    if !fi.moves.is_empty() {
        ui.ctx().request_repaint();
    }
}
// <META - ROLE : the frame path's own contract | walk the states a user can
// actually reach — the view goes through `ViewStore`, which fits and clamps —
#[cfg(test)]
mod frame_buffer_tests {
    use super::*;
    use pixel_engine::view::ViewStore;

    fn params<'a>(
        view: pixel_engine::view::View,
        comp: &'a [u8],
        css_w: f64,
        css_h: f64,
    ) -> DisplayParams<'a> {
        DisplayParams {
            view,
            canvas_w: 2048,
            canvas_h: 1088,
            css_w,
            css_h,
            dpr: 1.0,
            background: "transparent",
            grid: GridMode::Off,
            comp: Some(comp),
        }
    }

    /// `draw_display_into` owns its own size: the shell may hand it a 1x1 buffer
    /// and it must end up correct. A draft of `render_frame` resized the buffer
    /// again "for safety", double-clearing 2.6 MB per frame for nothing.
    #[test]
    fn draw_display_into_sizes_its_own_framebuffer() {
        let mut store = ViewStore::new();
        store.set_canvas(2048, 1088).expect("valid canvas size");
        store.set_viewport(1040.0, 640.0);
        let comp = vec![0u8; 2048 * 1088 * 4];

        let mut fb = Framebuffer::new(1, 1); // exactly what shell.rs builds
        draw_display_into(&params(store.view(), &comp, 1040.0, 640.0), &mut fb);
        let (w, h) = (fb.width, fb.height);
        assert!(w > 1 && h > 1, "1x1 must have been resized, got {w}x{h}");
        assert_eq!(fb.rgba.len(), (w * h * 4) as usize);

        // Every frame after the first hands over an already-sized buffer.
        draw_display_into(&params(store.view(), &comp, 1040.0, 640.0), &mut fb);
        assert_eq!((fb.width, fb.height), (w, h));
    }

    /// The whole zoom ladder, every pan corner, both fit and 1:1, at three
    /// viewport sizes. `blit_visible` indexes `fb.rgba` directly with no
    /// framebuffer clamp, so any view where the canvas rect is wider or taller
    /// than the buffer would panic here.
    #[test]
    fn every_reachable_view_state_stays_inside_the_framebuffer() {
        let comp = vec![0u8; 2048 * 1088 * 4];
        for (vw, vh) in [(1040.0, 640.0), (900.0, 560.0), (1920.0, 1080.0)] {
            let mut store = ViewStore::new();
            store.set_canvas(2048, 1088).expect("valid canvas size");
            store.set_viewport(vw, vh);
            store.fit();
            for _ in 0..24 {
                for &(dx, dy) in &[(0.0, 0.0), (vw / 2.0, vh / 2.0), (-vw, -vh)] {
                    store.pan_by(dx, dy);
                    let mut fb = Framebuffer::new(1, 1);
                    draw_display_into(&params(store.view(), &comp, vw, vh), &mut fb);
                    assert_eq!(fb.rgba.len(), (fb.width * fb.height * 4) as usize);
                }
                store.zoom_in();
                let mut fb = Framebuffer::new(1, 1);
                draw_display_into(&params(store.view(), &comp, vw, vh), &mut fb);
                assert_eq!(fb.rgba.len(), (fb.width * fb.height * 4) as usize);
            }
            // And back down to 1:1 and out.
            store.actual_size();
            let mut fb = Framebuffer::new(1, 1);
            draw_display_into(&params(store.view(), &comp, vw, vh), &mut fb);
            assert_eq!(fb.rgba.len(), (fb.width * fb.height * 4) as usize);
        }
    }

    /// A viewport change must be followed, not ignored — this is the resize path
    /// the frame relies on when the window moves between monitors.
    #[test]
    fn draw_display_into_follows_a_viewport_change() {
        let mut store = ViewStore::new();
        store.set_canvas(2048, 1088).expect("valid canvas size");
        store.set_viewport(1040.0, 640.0);
        let comp = vec![0u8; 2048 * 1088 * 4];
        let mut fb = Framebuffer::new(1, 1);
        draw_display_into(&params(store.view(), &comp, 1040.0, 640.0), &mut fb);
        draw_display_into(&params(store.view(), &comp, 640.0, 480.0), &mut fb);
        assert_ne!(
            (fb.width, fb.height),
            (1040, 640),
            "the smaller viewport stuck"
        );
        assert_eq!(fb.rgba.len(), (fb.width * fb.height * 4) as usize);
    }
}
