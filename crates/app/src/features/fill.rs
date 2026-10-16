//! `features/fill/fill.js` — the paint bucket.
//!
//! One `stroke_with` call runs `kernels::flood_fill_scanline` to completion;
//! the engine owns the algorithm, the tool only opens the stroke and picks the
//! seed/fill colours. Same-colour seed notifies `"이미 같은 색입니다"` and
//! cancels the stroke it just opened.

use pixel_engine::kernels::flood_fill_scanline;
use pixel_engine::model::edit_session::EditOptions;
use pixel_engine::pixel::{pack_rgba, parse_hex};

use crate::input::{Button, ToolEvent};
use crate::tools::api::{CursorKind, Tool, ToolCx};

pub struct FillTool;

impl FillTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FillTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for FillTool {
    fn id(&self) -> &'static str {
        "fill"
    }

    fn cursor(&self) -> CursorKind {
        CursorKind::Fill
    }

    fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if ev.button != Button::Left {
            return;
        }
        let Some(doc) = cx.session.doc() else {
            return;
        };
        let (w, h) = (doc.canvas.width_px, doc.canvas.height_px);
        if ev.x < 0 || ev.y < 0 || ev.x >= w as i32 || ev.y >= h as i32 {
            return;
        }
        if cx
            .session
            .stroke_begin(EditOptions::on_active_layer().labeled("페인트통"))
            .is_err()
        {
            return;
        }
        // The bounds above make these casts exact; the layer is the stroke's.
        let (sx, sy) = (ev.x as usize, ev.y as usize);
        let seed = cx
            .session
            .doc()
            .and_then(|d| d.get_layer(&d.active_layer_id).ok())
            .map_or(0, |l| l.store.get_pixel(ev.x as u32, ev.y as u32));
        let fill = match parse_hex(&cx.session.settings().primary_color) {
            Some(c) => pack_rgba(c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64),
            None => pack_rgba(0.0, 0.0, 0.0, 255.0),
        };
        if seed == fill {
            cx.session
                .notify("info", "이미 같은 색입니다", "FILL_SAME_COLOR");
            cx.session.stroke_cancel();
            return;
        }
        let _ = cx.session.stroke_with(|writer| {
            flood_fill_scanline(writer, w as usize, h as usize, sx, sy, seed, fill);
        });
        let _ = cx.session.stroke_commit(Some("페인트통"));
    }
}
