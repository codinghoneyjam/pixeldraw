//! `features/text/text_tool.js` — the bitmap-glyph text stamp.
//!
//! No drag state and no overlay: `pointer_down` stamps the whole string in
//! one stroke (the JS paints the full-canvas mask inside `pointerDown`).
//! `Math.round(ev.x)` is the identity here — `ToolEvent.x/y` are already the
//! floored pixel, and the JS rounds that same integer.
//! `strokeMode` defaults to `"shape"` in the JS (`?? "shape"`), hence
//! `shape_stroke: true` below.

use std::collections::BTreeMap;

use pixel_engine::model::edit_session::EditOptions;
use pixel_engine::pixel::{pack_rgba, parse_hex};
use pixel_engine::raster::text_mask::{GlyphSet, TextParams, decode_glyph_sets, text_mask};

use crate::input::{Button, ToolEvent};
use crate::tools::api::{CursorKind, Tool, ToolCx};

// Same committed pack the recipe renderer reads (`recipe/text.rs:10`);
// that second reader exists, so this file must stay in Phase 6.
const GLYPH_JSON: &str = include_str!("../../../../tools/recipe/text_glyphs.json");

/// Decode the committed pack; a corrupt pack yields an empty map so every
/// font lookup warns `TEXT_FONT_MISSING` instead of panicking.
pub fn load_glyph_sets() -> BTreeMap<String, GlyphSet> {
    let raw: serde_json::Value = serde_json::from_str(GLYPH_JSON).unwrap_or_default();
    decode_glyph_sets(&raw).unwrap_or_default()
}

fn pack_of(css: &str) -> u32 {
    match parse_hex(css) {
        Some(c) => pack_rgba(c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64),
        None => pack_rgba(0.0, 0.0, 0.0, 255.0),
    }
}

pub struct TextTool {
    text: String,
    font: String,
    stroke_width: i32,
    glyph_sets: BTreeMap<String, GlyphSet>,
}

impl TextTool {
    pub fn new() -> Self {
        Self::with_glyph_sets(load_glyph_sets())
    }

    pub fn with_glyph_sets(glyph_sets: BTreeMap<String, GlyphSet>) -> Self {
        Self {
            text: String::new(),
            font: "default-42".to_string(),
            stroke_width: 0,
            glyph_sets,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// `panel_text_options.js`: the text input applies on every keystroke.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn font(&self) -> &str {
        &self.font
    }

    pub fn set_font(&mut self, font: impl Into<String>) {
        self.font = font.into();
    }

    pub fn stroke_width(&self) -> i32 {
        self.stroke_width
    }

    /// Integer widths clamp to 0..16.
    pub fn set_stroke_width(&mut self, width: i32) -> bool {
        let clamped = width.clamp(0, 16);
        let changed = self.stroke_width != clamped;
        self.stroke_width = clamped;
        changed
    }

    /// `panel_text_options.js`: `Number.isInteger` widths clamp, anything
    /// else resets to 0 (the previous value is not kept).
    pub fn set_stroke_width_number(&mut self, width: f64) -> bool {
        if width.is_finite() && width.fract() == 0.0 {
            self.set_stroke_width(width as i32)
        } else {
            let changed = self.stroke_width != 0;
            self.stroke_width = 0;
            changed
        }
    }
}

impl Default for TextTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TextTool {
    fn id(&self) -> &'static str {
        "text"
    }

    fn cursor(&self) -> CursorKind {
        CursorKind::Text
    }

    fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if ev.button != Button::Left {
            return;
        }
        if self.text.is_empty() {
            return;
        }
        let Some(doc) = cx.session.doc() else {
            return;
        };
        let (w, h) = (doc.canvas.width_px as usize, doc.canvas.height_px as usize);
        let Some(set) = self.glyph_sets.get(&self.font) else {
            cx.session.notify(
                "warn",
                "텍스트 글꼴을 불러오지 못했습니다",
                "TEXT_FONT_MISSING",
            );
            return;
        };
        if let Some(ch) = self.text.chars().find(|c| !set.glyphs.contains_key(c)) {
            cx.session.notify(
                "warn",
                format!("지원하지 않는 문자: {ch}"),
                "TEXT_GLYPH_MISSING",
            );
            return;
        }
        let settings = cx.session.settings();
        let params = TextParams {
            text: &self.text,
            glyphs: &set.glyphs,
            x: ev.x,
            y: ev.y,
            fill: pack_of(&settings.primary_color),
            stroke: pack_of(&settings.secondary_color),
            stroke_width: self.stroke_width,
            shape_stroke: true,
            w,
            h,
        };
        let mask = match text_mask(&params) {
            Ok(m) => m,
            Err(e) => {
                cx.report(e);
                return;
            }
        };
        if cx
            .session
            .stroke_begin(EditOptions::on_active_layer().labeled("텍스트"))
            .is_err()
        {
            return;
        }
        let _ = cx.session.stroke_with(|wr| {
            for (i, cell) in mask.data.iter().enumerate() {
                if *cell != 0 {
                    wr.set((i % w) as u32, (i / w) as u32, *cell);
                }
            }
        });
        let _ = cx.session.stroke_commit(Some("텍스트"));
        cx.request_render();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::rc::Rc;

    use pixel_engine::model::{ModelEvent, NewDocumentOptions, Session, SettingInput};
    use pixel_engine::raster::text_mask::Glyph;
    use pixel_engine::view::View;

    use crate::input::{RawPointer, build_tool_event};
    use crate::tools::api::{CursorKind, Tool, ToolCx};

    use super::{TextTool, load_glyph_sets};

    type Queue = Rc<RefCell<VecDeque<ModelEvent>>>;

    fn session64() -> (Session, Queue) {
        let mut session = Session::new();
        session
            .new_document(NewDocumentOptions::default().size(64, 64))
            .expect("new document");
        let queue: Queue = Rc::new(RefCell::new(VecDeque::new()));
        let sink = Rc::clone(&queue);
        session.subscribe(move |ev| {
            sink.borrow_mut().push_back(ev.clone());
            Ok(())
        });
        (session, queue)
    }

    fn view1() -> View {
        View::new(1.0, 0.0, 0.0).expect("valid view")
    }

    fn tap(tool: &mut TextTool, session: &mut Session, x: f64, y: f64) -> bool {
        let mut render = false;
        let view = view1();
        let mut cx = ToolCx {
            session,
            view,
            need_render: &mut render,
        };
        tool.pointer_down(&build_tool_event(&RawPointer::new((x, y)), &view), &mut cx);
        render
    }

    fn painted(session: &Session) -> HashSet<(i32, i32)> {
        let doc = session.doc().expect("doc");
        let layer = doc.get_layer(&doc.active_layer_id).expect("active");
        let mut out = HashSet::new();
        for y in 0..64 {
            for x in 0..64 {
                if layer.store.get_pixel(x, y) != 0 {
                    out.insert((x as i32, y as i32));
                }
            }
        }
        out
    }

    fn one_px_sets() -> std::collections::BTreeMap<String, pixel_engine::raster::text_mask::GlyphSet>
    {
        let mut glyphs = HashMap::new();
        glyphs.insert(
            'A',
            Glyph {
                advance: 1.0,
                dx: 0,
                dy: 0,
                w: 1,
                h: 1,
                alpha: vec![255],
                shaped: Default::default(),
            },
        );
        let mut sets = std::collections::BTreeMap::new();
        sets.insert(
            "default-42".to_string(),
            pixel_engine::raster::text_mask::GlyphSet {
                path: String::new(),
                size: 42.0,
                glyphs,
            },
        );
        sets
    }

    fn status(queue: &Queue) -> Vec<(String, String, String)> {
        queue
            .borrow()
            .iter()
            .filter_map(|ev| match &ev.detail {
                pixel_engine::model::event::EventDetail::Status { level, text, code } => {
                    Some((level.to_string(), text.to_string(), code.to_string()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn defaults_match_the_js_constructor_and_the_pack_loads() {
        let tool = TextTool::with_glyph_sets(one_px_sets());
        assert_eq!(tool.id(), "text");
        assert_eq!(tool.cursor(), CursorKind::Text);
        assert_eq!(tool.text(), "");
        assert_eq!(tool.font(), "default-42");
        assert_eq!(tool.stroke_width(), 0);
        let bundled = load_glyph_sets();
        assert!(
            bundled.contains_key("default-42"),
            "the committed pack decodes"
        );
    }

    #[test]
    fn one_letter_commits_labelled_text_and_undo_restores() {
        let (mut session, _queue) = session64();
        let mut tool = TextTool::with_glyph_sets(one_px_sets());
        tool.set_text("A");
        assert!(tap(&mut tool, &mut session, 10.0, 10.0));
        assert!(!session.has_stroke(), "stamp commits inside pointer_down");
        assert_eq!(painted(&session), HashSet::from([(10, 10)]));
        assert_eq!(
            session.history().undo_label().as_deref(),
            Some("텍스트"),
            "the JS history label, verbatim"
        );
        assert!(session.undo().expect("undo"));
        assert!(painted(&session).is_empty(), "undo restores transparency");
    }

    #[test]
    fn empty_text_and_other_buttons_are_ignored() {
        let (mut session, _queue) = session64();
        let mut tool = TextTool::with_glyph_sets(one_px_sets());
        assert!(!tap(&mut tool, &mut session, 10.0, 10.0));
        assert!(painted(&session).is_empty());
        tool.set_text("A");
        let mut render = false;
        let view = view1();
        let mut ev = build_tool_event(&RawPointer::new((10.0, 10.0)), &view);
        ev.button = crate::input::Button::Right;
        let mut cx = ToolCx {
            session: &mut session,
            view,
            need_render: &mut render,
        };
        tool.pointer_down(&ev, &mut cx);
        assert!(painted(&session).is_empty(), "right button paints nothing");
        assert!(
            !session.undo().expect("undo"),
            "no history entry for ignored taps"
        );
    }

    #[test]
    fn missing_font_warns_with_the_js_code() {
        let (mut session, queue) = session64();
        let mut tool = TextTool::with_glyph_sets(one_px_sets());
        tool.set_text("A");
        tool.set_font("no-such-font");
        tap(&mut tool, &mut session, 10.0, 10.0);
        assert!(painted(&session).is_empty());
        assert!(
            status(&queue)
                .iter()
                .any(|(level, text, code)| level == "warn"
                    && text == "텍스트 글꼴을 불러오지 못했습니다"
                    && code == "TEXT_FONT_MISSING"),
            "font warning fired"
        );
    }

    #[test]
    fn unsupported_char_warns_naming_the_first_one() {
        let (mut session, queue) = session64();
        let mut tool = TextTool::with_glyph_sets(one_px_sets());
        tool.set_text("AB");
        tap(&mut tool, &mut session, 10.0, 10.0);
        assert!(painted(&session).is_empty());
        assert!(
            status(&queue)
                .iter()
                .any(|(level, text, code)| level == "warn"
                    && text == "지원하지 않는 문자: B"
                    && code == "TEXT_GLYPH_MISSING"),
            "glyph warning fired"
        );
    }

    #[test]
    fn stroke_width_option_follows_the_panel_rule() {
        let mut tool = TextTool::with_glyph_sets(one_px_sets());
        assert!(tool.set_stroke_width_number(5.0));
        assert_eq!(tool.stroke_width(), 5);
        assert!(tool.set_stroke_width_number(20.0));
        assert_eq!(tool.stroke_width(), 16, "integers clamp to 0..16");
        assert!(tool.set_stroke_width_number(-2.0));
        assert_eq!(tool.stroke_width(), 0);
        tool.set_stroke_width(4);
        assert!(tool.set_stroke_width_number(3.5));
        assert_eq!(tool.stroke_width(), 0, "non-integers reset, not keep");
        assert!(!tool.set_stroke_width_number(f64::NAN));
        assert_eq!(tool.stroke_width(), 0);
    }

    #[test]
    fn stroke_width_paints_a_disk_outline() {
        let (mut plain_session, _) = session64();
        let mut plain_tool = TextTool::with_glyph_sets(one_px_sets());
        plain_tool.set_text("A");
        tap(&mut plain_tool, &mut plain_session, 32.0, 32.0);
        assert_eq!(painted(&plain_session).len(), 1, "fill stamps one cell");

        let (mut session, _) = session64();
        let mut tool = TextTool::with_glyph_sets(one_px_sets());
        tool.set_text("A");
        tool.set_stroke_width(1);
        tap(&mut tool, &mut session, 32.0, 32.0);
        assert_eq!(painted(&session).len(), 5, "the r=1 disk stamps 5 cells");
        assert!(session.undo().expect("undo"));
        assert!(painted(&session).is_empty());
    }

    #[test]
    fn stamp_outside_the_canvas_clips_to_nothing() {
        let (mut session, _queue) = session64();
        let mut tool = TextTool::with_glyph_sets(one_px_sets());
        tool.set_text("A");
        tap(&mut tool, &mut session, 1000.0, 1000.0);
        assert!(painted(&session).is_empty());
        assert!(
            !session.undo().expect("undo"),
            "an empty stamp records no history"
        );
    }

    #[test]
    fn set_setting_colours_are_used() {
        let (mut session, _queue) = session64();
        session
            .set_setting("primaryColor", SettingInput::Str("#ff0000"))
            .expect("primary");
        let mut tool = TextTool::with_glyph_sets(one_px_sets());
        tool.set_text("A");
        tap(&mut tool, &mut session, 20.0, 20.0);
        let doc = session.doc().expect("doc");
        let layer = doc.get_layer(&doc.active_layer_id).expect("active");
        assert_eq!(
            layer.store.get_pixel(20, 20),
            pixel_engine::pixel::pack_rgba(255.0, 0.0, 0.0, 255.0)
        );
    }
}
