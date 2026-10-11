//! `<META - FILE SUMMARY - H3 headless e2e, ported from tests/e2e_session.test.mjs>`
//!
//! **Scope of this port.** Four of the nine JS groups are here, because they are
//! the ones a `Session` + the already-ported engine can express. The other five
//! need pieces that do not exist yet:
//!
//! | H3 | JS needs | status |
//! |---|---|---|
//! | H3.1/H3.2 | `ToolManager`, `PenTool` | Phase 2.7 — a stroke is driven here through `beginEdit` + the brush kernels instead |
//! | H3.3/H3.5 | `documentToJson`, `jsonToDocument`, `layerToJson`, `importLayerJson` | Phase 1.13 (`io/serialize`) |
//! | H3.7 | `flattenToRgba`, `exportPngBytes` | Phase 1.13; `flatten` is re-implemented here from `composite_chunk`, which is what it called |
//!
//! What follows asserts the same *document-model* facts the JS asserted — the
//! dirty flag's lifecycle, the history fingerprint surviving a rejected stroke,
//! the shape round-trip, and the byte-budget eviction — using the same numbers.

use pixel_engine::constants::CHUNK_LEN;
use pixel_engine::model::{Document, EditOptions, NewDocumentOptions, Session, SettingInput};
use pixel_engine::pixel::pack_rgba;
use pixel_engine::raster::brush::{stroke_point, stroke_segment};
use pixel_engine::raster::{FillMode, ShapeKind, ShapeSpec, apply_shape};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::rc::Rc;

/// `diag(x0, y0, x1, y1, step)` — the sample points of a diagonal stroke.
fn diag(x0: i32, y0: i32, x1: i32, y1: i32, step: i32) -> Vec<(i32, i32)> {
    let n = ((x1 - x0).abs().max((y1 - y0).abs()) as f64 / step as f64)
        .ceil()
        .max(1.0) as i32;
    let mut pts = Vec::with_capacity(n as usize + 1);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        pts.push((
            (x0 as f64 + (x1 - x0) as f64 * t).round() as i32,
            (y0 as f64 + (y1 - y0) as f64 * t).round() as i32,
        ));
    }
    pts
}

/// `hashDoc(doc)` — sha256 of the flattened RGBA.
///
/// The JS hashed `flattenToRgba(doc)`; that function is Phase 1.13, so the
/// flatten is rebuilt here from `composite_chunk`, the exact kernel it called.
fn flatten(doc: &Document) -> Vec<u8> {
    let w = doc.canvas.width_px;
    let h = doc.canvas.height_px;
    let sources = doc.layer_sources();
    let mut out = vec![0u8; (w * h * 4) as usize];
    for cy in 0..h / 32 {
        for cx in 0..w / 32 {
            let mut buf = [0u8; CHUNK_LEN];
            pixel_engine::composite::composite_chunk(&sources, cx, cy, &mut buf)
                .expect("valid chunk buffer");
            for row in 0..32u32 {
                let dst = ((cy * 32 + row) * w + cx * 32) as usize * 4;
                let src = row as usize * 128;
                out[dst..dst + 128].copy_from_slice(&buf[src..src + 128]);
            }
        }
    }
    out
}

fn hash_doc(doc: &Document) -> String {
    format!("{:x}", Sha256::digest(flatten(doc)))
}

/// `histFp(session)` — the history fingerprint.
#[derive(Debug, PartialEq)]
struct HistFp {
    can_undo: bool,
    can_redo: bool,
    undo_label: Option<String>,
    redo_label: Option<String>,
}

fn hist_fp(session: &Session) -> HistFp {
    let state = session.history().state();
    HistFp {
        can_undo: state.can_undo,
        can_redo: state.can_redo,
        undo_label: state.undo_label,
        redo_label: state.redo_label,
    }
}

/// `makePenSession(w, h)` — a session with a canvas and a pen.
fn pen_session(w: u32, h: u32) -> Session {
    let mut session = Session::new();
    session
        .new_document(NewDocumentOptions::default().size(w, h))
        .expect("new");
    session
}

/// `stroke(mgr, pts)` — the pen tool's pointer-down/move/up sequence.
///
/// `pointerDown` opens an edit and stamps the first point, `pointerMove` runs a
/// Bresenham segment per coalesced point, `pointerUp` commits.
fn stroke(session: &mut Session, pts: &[(i32, i32)], pen: usize, packed: u32) {
    let mut edit = session.begin_edit(EditOptions::default()).expect("edit");
    let writer = edit.writer().expect("editing");
    if pts.len() == 1 {
        stroke_point(writer, pts[0].0, pts[0].1, pen, packed);
    } else {
        for pair in pts.windows(2) {
            stroke_segment(
                writer, pair[0].0, pair[0].1, pair[1].0, pair[1].1, pen, packed,
            );
        }
    }
    assert!(edit.commit(None).expect("commit"));
}

// <META - ROLE : H3.4 dirty flag lifecycle | L106-118>
#[test]
fn dirty_flag_lifecycle() {
    let mut session = pen_session(64, 64);
    session.history_mut().mark_saved();
    assert!(!session.history().is_dirty());
    session
        .set_setting("primaryColor", SettingInput::Str("#ff0000"))
        .expect("color");
    stroke(&mut session, &diag(5, 5, 40, 40, 8), 1, 0xff0000ff);
    assert!(session.history().is_dirty());
    assert!(session.undo().expect("undo"));
    assert!(!session.history().is_dirty());
}

// <META - ROLE : H3.6 shapes x 4 kinds x 2 fills, undo restores the hash | L143-168>
#[test]
fn shape_undo_restores_the_flattened_hash() {
    for kind in ["line", "rect", "rrect", "ellipse"] {
        for fill in ["outline", "fill"] {
            let mut session = pen_session(64, 64);
            session
                .set_setting("primaryColor", SettingInput::Str("#ff0000"))
                .expect("color");
            session
                .set_setting("secondaryColor", SettingInput::Str("#0000ff"))
                .expect("color");
            session
                .set_setting("shapeFill", SettingInput::Str(fill))
                .expect("fill");
            session
                .set_setting("penSize", SettingInput::Num(3.0))
                .expect("pen");
            session
                .set_setting("shapeRadius", SettingInput::Num(3.0))
                .expect("radius");

            let spec = ShapeSpec {
                kind: ShapeKind::parse(kind).expect("kind"),
                stroke_width: 3,
                fill_mode: FillMode::parse(Some(fill)),
                bbox: Some([4, 4, 8, 7]),
                p0: Some((4, 4)),
                p1: Some((11, 10)),
                radius: 3,
                points: Vec::new(),
            };
            let pre = hash_doc(session.doc().expect("doc"));
            let mut edit = session.begin_edit(EditOptions::default()).expect("edit");
            let _ = apply_shape(
                edit.writer().expect("editing"),
                &spec,
                0xff0000ff,
                0xffff0000,
            )
            .expect("valid spec");
            assert!(edit.commit(None).expect("commit"));

            assert!(session.history().can_undo());
            let post = hash_doc(session.doc().expect("doc"));
            assert_ne!(post, pre, "{kind}/{fill} must change the canvas");
            assert!(session.undo().expect("undo"));
            assert_eq!(
                hash_doc(session.doc().expect("doc")),
                pre,
                "{kind}/{fill} undo must restore the hash"
            );
        }
    }
}

// <META - ROLE : H3.8 locked/hidden stroke rejected, pixels and history invariant | L203-230>
#[test]
fn a_locked_or_hidden_layer_rejects_the_stroke_and_stays_invariant() {
    let mut session = pen_session(64, 64);
    let notes = Rc::new(RefCell::new(Vec::<String>::new()));
    let inner = Rc::clone(&notes);
    let sub = session.subscribe(move |e| {
        if let pixel_engine::model::EventDetail::Status { code, .. } = &e.detail {
            inner.borrow_mut().push((*code).to_string());
        }
        Ok(())
    });
    stroke(&mut session, &diag(5, 5, 20, 20, 8), 1, 0xff0000ff);
    let id = session.doc().expect("doc").active_layer_id.clone();

    session.set_layer_locked(&id, true).expect("lock");
    let before_doc = hash_doc(session.doc().expect("doc"));
    let before_hist = hist_fp(&session);
    let err = session
        .begin_edit(EditOptions::default())
        .expect_err("locked");
    assert_eq!(err.code, pixel_engine::errors::CODE_LAYER_LOCKED);
    assert_eq!(hash_doc(session.doc().expect("doc")), before_doc);
    assert_eq!(hist_fp(&session), before_hist);
    assert!(!session.is_editing());
    assert!(notes.borrow().iter().any(|c| c == "LAYER_LOCKED"));

    session.set_layer_locked(&id, false).expect("unlock");
    session.set_layer_visible(&id, false).expect("hide");
    let before_doc = hash_doc(session.doc().expect("doc"));
    let before_hist = hist_fp(&session);
    let err = session
        .begin_edit(EditOptions::default())
        .expect_err("hidden");
    assert_eq!(err.code, pixel_engine::errors::CODE_LAYER_HIDDEN);
    assert_eq!(hash_doc(session.doc().expect("doc")), before_doc);
    assert_eq!(hist_fp(&session), before_hist);
    assert!(notes.borrow().iter().any(|c| c == "LAYER_HIDDEN"));
    session.unsubscribe(sub);
}

// <META - ROLE : H3.9 small-budget history evicts the oldest, keeps a minimum | L232-254>
#[test]
fn a_small_byte_budget_evicts_the_oldest_and_keeps_one() {
    let mut session = pen_session(512, 512);
    // `session.history.limitBytes = 2 * 1024 * 1024;`
    session.history_mut().limit_bytes = 2 * 1024 * 1024;
    let palette = [0xff0000ff, 0xff00ff00, 0xffff0000];
    for i in 0..12 {
        let pts = if i % 2 == 0 {
            diag(0, 0, 511, 511, 16)
        } else {
            diag(511, 0, 0, 511, 16)
        };
        stroke(&mut session, &pts, 64, palette[i % palette.len()]);
    }
    assert!(session.history().can_undo());
    let mut n = 0;
    while session.history().can_undo() {
        assert!(session.undo().expect("undo"));
        n += 1;
    }
    assert!(n >= 1, "min 1 retained");
    assert!(n < 12, "oldest evicted, kept {n}/12");
}

/// The pen colours the JS used, pinned so a mistaken packing shows up here.
///
/// The packing is `a<<24 | b<<16 | g<<8 | r`, so green is `0xff00ff00` — my
/// first attempt at this table used `0x00ff00ff` and the assertion caught it.
#[test]
fn pen_colours_are_packed_the_way_the_js_packed_them() {
    assert_eq!(pack_rgba(255.0, 0.0, 0.0, 255.0), 0xff0000ff);
    assert_eq!(pack_rgba(0.0, 255.0, 0.0, 255.0), 0xff00ff00);
    assert_eq!(pack_rgba(0.0, 0.0, 255.0, 255.0), 0xffff0000);
}
