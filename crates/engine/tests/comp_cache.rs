//! `<META - FILE SUMMARY - H3 comp_cache integration: dirty chunks, event table, 6 ms budget>`
//!
//! Ported from `src/render/renderer_composite.js` + the invalidation half of
//! `src/render/renderer.js`. The drive-through runs a real `Session`: every write
//! goes through the stroke API, the events it dispatches are collected by the
//! subscription queue, and that queue is what feeds `CompCache::apply_event` —
//! the same wiring W8 will use in its frame loop.
//!
//! The budget is asserted twice: wall clock (`Duration::ZERO`,
//! `Duration::from_secs(3600)`) for the JS path, and the deterministic
//! `Budget::Chunks` for the cut-off itself, because a `Duration` budget's
//! break-point depends on the machine (docs/roadmap/index.yaml R12).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use pixel_engine::comp_cache::{Budget, CompCache};
use pixel_engine::composite::sample_pixel;
use pixel_engine::constants::{CHUNK_PX, is_valid_canvas_size};
use pixel_engine::io::export::{FlattenOptions, flatten_to_rgba};
use pixel_engine::model::{
    Document, EditOptions, EventDetail, ModelEvent, NewDocumentOptions, Session,
};
use pixel_engine::raster::brush::stroke_point;
use pixel_engine::view::{View, VisibleChunkRange, visible_chunk_range};

/// The pen red the JS tests used — `a<<24 | b<<16 | g<<8 | r`, so `0x0000ffff`
/// would be *transparent* and drop out of the composite.
const RED: u32 = 0xff0000ff;
/// Bigger than any frame's work, so a "paint everything" call never cuts off.
const HUGE: Duration = Duration::from_secs(3600);

/// The W8 event queue: `Rc<RefCell<VecDeque<ModelEvent>>>`.
///
/// A `Session::subscribe` closure is `'static` and cannot touch the session or a
/// tool, so the JS listener's `comp_cache.apply_event` becomes a push here and
/// the drain happens in the frame loop.
type Queue = Rc<RefCell<VecDeque<ModelEvent>>>;

/// `session.subscribe(...)` — the JS `addEventListener(EVENTS.X, fn)` pair.
fn attach_queue(session: &mut Session) -> Queue {
    let queue: Queue = Rc::new(RefCell::new(VecDeque::new()));
    let sink = Rc::clone(&queue);
    session.subscribe(move |ev| {
        sink.borrow_mut().push_back(ev.clone());
        Ok(())
    });
    queue
}

/// The frame-loop drain: dispatch order preserved, queue left empty.
fn drain(queue: &Queue) -> Vec<ModelEvent> {
    queue.borrow_mut().drain(..).collect()
}

/// One frame's worth of invalidation + compositing, the way `_update` runs it.
///
/// Returns whether the display must upload a new texture: an event asked for a
/// frame, dirty chunks remain for the next frame, or this frame painted
/// anything. (`update` alone only reports remaining dirt — the budget tests pin
/// that — so the painted signal is OR'd in here. W8's frame loop repeats this.)
fn pump(
    cache: &mut CompCache,
    doc: &Document,
    queue: &Queue,
    visible: Option<VisibleChunkRange>,
) -> bool {
    let mut need_render = false;
    for ev in drain(queue) {
        need_render |= cache.apply_event(&ev);
    }
    if cache.update(doc, visible, HUGE) {
        need_render = true;
    }
    need_render || cache.last_painted() > 0
}

fn dot(session: &mut Session, x: i32, y: i32, size: usize, packed: u32) {
    session
        .stroke_begin(EditOptions::default())
        .expect("stroke begins on a fresh layer");
    session
        .stroke_with(|w| stroke_point(w, x, y, size, packed))
        .expect("stroke writes");
    session.stroke_flush();
    session.stroke_commit(None).expect("stroke commits");
}

fn canvas(w: u32, h: u32) -> (Session, Queue) {
    let mut session = Session::new();
    session
        .new_document(NewDocumentOptions::default().size(w, h))
        .expect("new document");
    let queue = attach_queue(&mut session);
    (session, queue)
}

/// A `Document` with no session behind it — the cache only needs its layers.
fn doc(w: u32, h: u32) -> Document {
    pixel_engine::model::new_document(NewDocumentOptions::default().size(w, h))
        .expect("new document")
}

/// A cache whose first frame finished: nothing dirty, nothing left to paint.
fn clean_cache(d: &Document) -> CompCache {
    let mut cache = CompCache::new();
    assert!(!cache.update(d, None, HUGE));
    assert_eq!(cache.dirty_len(), 0);
    cache
}

/// The 2×2-chunk rectangle a 64×64 viewport shows of a 128×64 canvas at zoom 1.
fn view() -> View {
    View::new(1.0, 0.0, 0.0).expect("valid view")
}

fn visible_of(v: View, w: u32, h: u32, vw: f64, vh: f64) -> Option<VisibleChunkRange> {
    visible_chunk_range(v, w as f64, h as f64, vw, vh)
}

/// Byte-for-byte the same composite the file exporter would write, which is the
/// ground truth `comp` has to equal (both composite over a transparent base).
fn flatten(doc: &Document) -> Vec<u8> {
    flatten_to_rgba(
        &doc.layer_sources(),
        doc.canvas.width_px,
        doc.canvas.height_px,
        FlattenOptions {
            include_background: false,
        },
    )
}

// <META - ROLE : W3 test 1 — one dot, every pixel swept against sample_pixel | L1-40>
#[test]
fn one_dot_composites_exactly_where_sample_pixel_sees_it() {
    let (mut session, queue) = canvas(64, 64);
    let mut cache = CompCache::new();
    // The first frame paints nothing yet: the cache starts fully dirty.
    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));
    assert_eq!(cache.size(), (64, 64));
    assert!(is_valid_canvas_size(64, 64));
    assert!(
        cache.comp().iter().all(|&b| b == 0),
        "unpainted is transparent"
    );

    dot(&mut session, 20, 11, 3, RED);
    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));

    let doc = session.doc().expect("doc");
    let sources = doc.layer_sources();
    let (w, h) = cache.size();
    assert_eq!(cache.comp().len(), (w * h * 4) as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let expected = sample_pixel(&sources, x, y, w as i32, h as i32);
            let o = ((y * w as i32 + x) * 4) as usize;
            assert_eq!(
                &cache.comp()[o..o + 4],
                &expected,
                "comp pixel ({x},{y}) must equal sample_pixel"
            );
        }
    }
    assert_eq!(
        cache.comp(),
        flatten(doc),
        "comp equals the flattened document"
    );
}

// <META - ROLE : W3 test 2 — an empty dirty set composites nothing | L42-56>
#[test]
fn a_second_frame_with_no_changes_paints_nothing() {
    let (mut session, queue) = canvas(64, 64);
    let mut cache = CompCache::new();
    dot(&mut session, 20, 11, 3, RED);
    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));
    let painted = cache.last_painted();
    assert!(painted > 0, "the dot dirtied chunk (0,0)");

    assert!(drain(&queue).is_empty(), "no event, no invalidation");
    assert!(!pump(&mut cache, session.doc().expect("doc"), &queue, None));
    assert_eq!(cache.dirty_len(), 0);
    assert_eq!(cache.last_painted(), 0);
    assert_eq!(cache.comp(), flatten(session.doc().expect("doc")));
}

// <META - ROLE : W3 test 3 — the six-row event table | L58-104>
#[test]
fn the_event_table_is_js_exact() {
    let d = doc(128, 64);
    let grid = 4 * 2;

    // Row 1 — `PixelsAll` → invalidate_all, frame requested.
    let mut cache = clean_cache(&d);
    assert!(cache.apply_event(&ModelEvent::pixels_all_changed("layer-0001")));
    assert_eq!(cache.dirty_len(), grid);

    // Row 2 — `PixelsChunks` → only those chunks, out-of-canvas ones dropped.
    let mut cache = clean_cache(&d);
    assert!(cache.apply_event(&ModelEvent::pixels_changed("layer-0001", vec![(3, 1)])));
    assert_eq!(cache.dirty_len(), 1);
    assert!(
        cache.apply_event(&ModelEvent::pixels_changed(
            "layer-0001",
            vec![(9, 9), (4, 0)]
        )),
        "a chunk list of nothing but off-canvas keys still asks for a frame"
    );
    assert_eq!(cache.dirty_len(), 1, "out-of-canvas chunks are ignored");

    // Row 3 — `Layers{Active}` → ignore, no frame, no invalidation.
    let mut cache = clean_cache(&d);
    let active = ModelEvent::layers_changed(pixel_engine::model::LayerChangeReason::Active, vec![]);
    assert!(!cache.apply_event(&active));
    assert_eq!(cache.dirty_len(), 0);

    // Row 4 — `Layers{Prop}` → invalidate_all + frame. The JS also skipped
    // `prop` + `field === "name"`, but no event carries a field name, so `prop`
    let mut cache = clean_cache(&d);
    let prop = ModelEvent::layers_changed(pixel_engine::model::LayerChangeReason::Prop, vec![]);
    assert!(cache.apply_event(&prop));
    assert_eq!(cache.dirty_len(), grid);
    let move_up = ModelEvent::layers_changed(pixel_engine::model::LayerChangeReason::Move, vec![]);
    assert!(cache.apply_event(&move_up));
    assert_eq!(cache.dirty_len(), grid);

    // Row 5 — `Settings{GridMode}` → frame, no invalidation.
    let mut cache = clean_cache(&d);
    let grid_mode = ModelEvent::settings_changed(
        pixel_engine::model::SettingKey::GridMode,
        pixel_engine::model::SettingValue::Mode("unit"),
    );
    assert!(cache.apply_event(&grid_mode));
    assert_eq!(cache.dirty_len(), 0, "the grid changes no composited pixel");

    // Row 6 — anything else, and any non-grid setting → ignored, no frame.
    let mut cache = clean_cache(&d);
    let pen = ModelEvent::settings_changed(
        pixel_engine::model::SettingKey::PenSize,
        pixel_engine::model::SettingValue::Int(2),
    );
    assert!(!cache.apply_event(&pen));
    assert!(!cache.apply_event(&ModelEvent::status_message("warn", "x", "LAYER_LOCKED")));
    assert!(!cache.apply_event(&ModelEvent::tool_state("rect", "{}")));
    assert_eq!(cache.dirty_len(), 0);
}

// <META - ROLE : W3 test 4 — the budget never throttles what is visible | L106-131>
#[test]
fn the_budget_skips_the_visible_chunks_entirely() {
    let d = doc(128, 64);
    let vis = visible_of(view(), 128, 64, 64.0, 64.0).expect("on screen");
    assert_eq!(
        (vis.cx0, vis.cx1, vis.cy0, vis.cy1),
        (0, 1, 0, 1),
        "a 64×64 viewport of a 128×64 canvas at zoom 1 sees 2×2 chunks"
    );
    let grid = 8;
    let on_screen = 4;

    let mut cache = clean_cache(&d);
    cache.invalidate_all();

    // Zero budget, deterministic: the four visible chunks paint anyway and the
    // off-screen pass stops before its first chunk.
    assert!(cache.update_budget(&d, Some(vis), Budget::Chunks(0)));
    assert_eq!(cache.last_painted(), on_screen);
    assert_eq!(cache.dirty_len(), grid - on_screen);

    // Two chunks a frame: the (cy, cx) order decides which ones go first.
    assert!(cache.update_budget(&d, Some(vis), Budget::Chunks(2)));
    assert_eq!(cache.last_painted(), 2);
    assert_eq!(cache.dirty_len(), grid - on_screen - 2);

    // …and the frame after that finishes the grid.
    assert!(!cache.update_budget(&d, Some(vis), Budget::Chunks(9)));
    assert_eq!(cache.last_painted(), grid - on_screen - 2);
    assert_eq!(cache.dirty_len(), 0);

    // Zero budget on the wall clock: the same visible pass, plus at most the one
    // off-screen chunk whose check read a clock that had not advanced yet (the
    let mut cache = clean_cache(&d);
    cache.invalidate_all();
    assert!(cache.update(&d, Some(vis), Duration::ZERO));
    let first_frame = cache.last_painted();
    assert!(first_frame >= on_screen, "the visible pass has no budget");
    assert!(
        first_frame <= on_screen + 1,
        "nothing off screen fits in a zero budget"
    );
    assert!(!cache.update(&d, Some(vis), HUGE));
    assert_eq!(cache.dirty_len(), 0);
    assert_eq!(cache.last_painted(), grid - first_frame);

    // The frame budget is the contract constant, not a magic number.
    assert_eq!(Budget::frame(), Budget::Time(Duration::from_millis(6)));
}

// <META - ROLE : W3 test 5 — a resize reallocates and dirties the whole canvas | L133-155>
#[test]
fn a_resize_reallocates_and_dirties_everything() {
    let (mut session, queue) = canvas(64, 64);
    let mut cache = CompCache::new();
    dot(&mut session, 5, 5, 1, RED);
    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));
    assert_eq!(cache.dirty_len(), 0);

    assert!(session.resize_canvas(128, 64).expect("resize"));
    let events = drain(&queue);
    assert!(
        events
            .iter()
            .any(|e| matches!(e.detail, EventDetail::PixelsAll { .. })),
        "a resize reports every layer as all-pixels-changed"
    );
    for ev in &events {
        cache.apply_event(ev);
    }
    // `dirty_all` counts the grid the buffer still has: the old 2×2. The new
    // 4×2 grid lands when `ensure` runs inside the next `update` — JS
    assert_eq!(
        cache.dirty_len(),
        4,
        "everything dirty against the old grid"
    );
    assert_eq!(cache.size(), (64, 64), "`ensure` has not run yet");

    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));
    assert_eq!(cache.size(), (128, 64));
    assert_eq!(cache.comp().len(), (128 * 64 * 4) as usize);
    assert_eq!(cache.dirty_len(), 0);
    assert_eq!(
        cache.comp(),
        flatten(session.doc().expect("doc")),
        "the dot survived the resize and was recomposited"
    );
}

// <META - ROLE : W3 test 6 — the W8 queue pattern end to end | L157-200>
#[test]
fn stroke_undo_redo_through_the_event_queue() {
    let (mut session, queue) = canvas(64, 64);
    let mut cache = CompCache::new();

    // ── the frame loop's order: drain, then composite ──
    dot(&mut session, 10, 10, 3, RED);
    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));
    assert_eq!(cache.comp(), flatten(session.doc().expect("doc")));
    assert_eq!(
        &cache.comp()[4 * (10 * 64 + 10)..][..4],
        &[0xff, 0, 0, 0xff]
    );

    assert!(session.undo().expect("undo"));
    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));
    assert_eq!(cache.dirty_len(), 0, "the undo repainted the whole canvas");
    assert!(
        cache.comp().iter().all(|&b| b == 0),
        "the dot is gone from comp as well"
    );

    assert!(session.redo().expect("redo"));
    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));
    assert_eq!(cache.comp(), flatten(session.doc().expect("doc")));
    assert_eq!(
        &cache.comp()[4 * (10 * 64 + 10)..][..4],
        &[0xff, 0, 0, 0xff]
    );

    // A hidden layer changes the composite without touching a single chunk.
    let id = session.doc().expect("doc").active_layer_id.clone();
    assert!(session.set_layer_visible(&id, false).expect("hide"));
    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));
    assert!(cache.comp().iter().all(|&b| b == 0));

    // A rename asks for a frame but changes no pixel, so comp is untouched.
    let before = cache.comp().to_vec();
    assert!(session.rename_layer(&id, "이름").expect("rename"));
    assert!(pump(&mut cache, session.doc().expect("doc"), &queue, None));
    assert_eq!(cache.comp(), before, "a name change repaints nothing");
}

/// The canvas-size assumption the row-copy relies on: chunk boundaries land
/// exactly on the buffer's edge, so no row is ever clipped.
#[test]
fn canvas_sizes_are_multiples_of_the_chunk() {
    for (w, h) in [(32, 32), (64, 64), (512, 512), (2048, 1088), (128, 32)] {
        assert!(
            is_valid_canvas_size(w, h),
            "{w}x{h} must be a legal canvas size"
        );
        assert_eq!(w % CHUNK_PX, 0, "{w} is a whole number of chunks");
        assert_eq!(h % CHUNK_PX, 0, "{h} is a whole number of chunks");
    }
    assert!(!is_valid_canvas_size(48, 32));
    assert!(!is_valid_canvas_size(32, 33));
}
