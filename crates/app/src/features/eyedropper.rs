//! `features/eyedropper/eyedropper.js` — colour sampling.
//!
//! Button map, exactly like the JS header: Left → primary, Left+Shift or Right
//! → secondary, any accepted button + Alt → the active layer's raw pixel
//! instead of the composite. The down button latches for the drag because a
//! move reports no button. Middle is ignored.

use pixel_engine::composite::sample_pixel;
use pixel_engine::model::SettingInput;
use pixel_engine::pixel::to_hex8;
use pixel_engine::view::canvas_to_screen;

use crate::input::{Button, HoverEvent, ToolEvent};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

pub struct EyedropperTool {
    down: bool,
    button: Button,
    pos: Option<(i32, i32)>,
}

impl EyedropperTool {
    pub fn new() -> Self {
        Self {
            down: false,
            button: Button::Left,
            pos: None,
        }
    }

    fn release(&mut self) {
        self.down = false;
        self.button = Button::Left;
    }

    /// Active-layer raw pixel (Alt) or the display composite. Out-of-canvas
    /// reads 0 either way — `get_pixel` and `sample_pixel` both return 0 there
    /// — so the `"투명 픽셀"` path below covers them, like the JS.
    fn pick(&self, ev: &ToolEvent, cx: &mut ToolCx) {
        let Some(doc) = cx.session.doc() else {
            return;
        };
        let rgba = if ev.alt {
            doc.get_layer(&doc.active_layer_id)
                .ok()
                .map(|l| {
                    let v = if ev.x < 0 || ev.y < 0 {
                        0
                    } else {
                        l.store.get_pixel(ev.x as u32, ev.y as u32)
                    };
                    pixel_engine::pixel::unpack_rgba(v)
                })
                .unwrap_or([0, 0, 0, 0])
        } else {
            sample_pixel(
                &doc.layer_sources(),
                ev.x,
                ev.y,
                doc.canvas.width_px as i32,
                doc.canvas.height_px as i32,
            )
        };
        if rgba[3] == 0 {
            cx.session.notify("info", "투명 픽셀", "TRANSPARENT_PIXEL");
            return;
        }
        let hex = to_hex8(
            rgba[0] as f64,
            rgba[1] as f64,
            rgba[2] as f64,
            rgba[3] as f64,
        );
        // The Shift rule wins even on a latched left drag — legacy behaviour.
        let secondary = ev.shift || self.button == Button::Right;
        let key = if secondary {
            "secondaryColor"
        } else {
            "primaryColor"
        };
        let _ = cx.session.set_setting(key, SettingInput::Str(&hex));
    }
}

impl Default for EyedropperTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for EyedropperTool {
    fn id(&self) -> &'static str {
        "eyedropper"
    }

    fn cursor(&self) -> CursorKind {
        CursorKind::Eyedropper
    }

    fn accepts_button(&self, button: Button) -> bool {
        matches!(button, Button::Left | Button::Right)
    }

    fn deactivate(&mut self, _cx: &mut ToolCx) {
        self.release();
    }

    fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if !self.accepts_button(ev.button) {
            return;
        }
        self.down = true;
        self.button = ev.button;
        self.pos = Some((ev.x, ev.y));
        self.pick(ev, cx);
    }

    fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if !self.down {
            return;
        }
        self.pos = Some((ev.x, ev.y));
        self.pick(ev, cx);
    }

    fn pointer_up(&mut self, _ev: &ToolEvent, _cx: &mut ToolCx) {
        self.release();
    }

    fn cancel(&mut self, _cx: &mut ToolCx) {
        self.release();
    }

    fn hover(&mut self, ev: Option<&HoverEvent>, cx: &mut ToolCx) {
        self.pos = ev.map(|h| (h.x, h.y));
        cx.request_render();
    }

    fn overlay(&self, info: &OverlayInfo, out: &mut Vec<OverlayOp>) {
        let Some((px, py)) = self.pos else { return };
        let p = canvas_to_screen(info.view, px as f64, py as f64);
        let w = (info.view.zoom() * info.dpr).max(1.0);
        out.push(OverlayOp::StrokeRect {
            x: p.x * info.dpr,
            y: p.y * info.dpr,
            w,
            h: w,
            rgba: [255, 255, 255, 255],
            dash: None,
            difference: true,
        });
    }
}
