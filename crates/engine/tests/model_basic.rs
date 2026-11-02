//! `<META - FILE SUMMARY - B5.1/B5.2/B5.3/B5.5, ported from tests/model_basic.test.mjs>`
//!
//! The document-model gate. Every assertion below is the JS one, rewritten
//! against the Rust API; the `<META - ROLE>` comments name the JS case so a
//! reader can diff the two.
//!
//! **Two adaptations the borrow checker forced**, both noted inline:
//!
//! 1. A listener has to be installed *before* `begin_edit`, because the handle
//!    borrows the whole `Session` for its lifetime. The JS could call
//!    `addEventListener` between `writer.set` and `commit`; here that would be a
//!    second `&mut` borrow. Nothing dispatches in that window anyway.
//! 2. `edit.commit()` after `edit.cancel()` is a **compile error**: `cancel`
//!    consumes the handle, so the JS's runtime `"edit is not active"` check has
//!    nothing left to guard.

use pixel_engine::chunkstore::{ChunkStore, PixelWriter};
use pixel_engine::constants::CHUNK_LEN;
use pixel_engine::events::{
    EVENT_DOCUMENT_REPLACED, EVENT_HISTORY_CHANGED, EVENT_LAYERS_CHANGED, EVENT_PIXELS_CHANGED,
    EVENT_SETTINGS_CHANGED, EVENT_STATUS_MESSAGE,
};
use pixel_engine::model::{
    AddLayerCommand, Command, Document, EditOptions, EventDetail, IdGen, Layer, LayerField,
    LayerPropValue, MoveLayerCommand, NewDocumentOptions, PaintCommand, RemoveLayerCommand,
    ResizeCanvasCommand, Session, SetLayerPropCommand, Subscription, add_layer, new_document,
};
use pixel_engine::pixel::pack_rgba;
use std::cell::RefCell;
use std::rc::Rc;

/// `assertDrawError(fn, code)`.
macro_rules! assert_draw_error {
    ($code:literal, $body:expr) => {
        match $body {
            Err(e) => assert_eq!(
                e.code, $code,
                "expected DrawToolError({}), got {}: {}",
                $code, e.code, e.message
            ),
            Ok(_) => panic!("expected DrawToolError({}), got Ok", $code),
        }
    };
}

/// `snapshotStore(store)` — deep copy of every populated chunk.
fn snapshot_store(store: &ChunkStore) -> Vec<(u32, u32, Vec<u8>)> {
    let mut out = Vec::new();
    store.for_each_chunk(|cx, cy, data| out.push((cx, cy, data.to_vec())));
    out
}

/// `paintPixel(doc, layerId, x, y, packed)`.
fn paint_pixel(doc: &mut Document, layer_id: &str, x: u32, y: u32, packed: u32) -> PaintCommand {
    let store = &mut doc.get_layer_mut(layer_id).expect("layer").store;
    let mut writer = PixelWriter::new(store, layer_id);
    writer.set(x, y, packed);
    let cs = writer.finish().expect("expected non-null changeset");
    PaintCommand::with_default_label(cs)
}

/// `collectEvents(session, names)` — an ordered `(name, detail)` recorder.
///
/// The subscription handle is handed back so the owner can unsubscribe; the
/// JS `done()` teardown works the same way.
struct Recorder {
    log: Rc<RefCell<Vec<(&'static str, EventDetail)>>>,
}

impl Recorder {
    fn new(session: &mut Session) -> (Self, Subscription) {
        let log: Rc<RefCell<Vec<(&'static str, EventDetail)>>> = Rc::new(RefCell::new(Vec::new()));
        let inner = Rc::clone(&log);
        let sub = session.subscribe(move |e| {
            inner.borrow_mut().push((e.name, e.detail.clone()));
            Ok(())
        });
        (Self { log }, sub)
    }

    /// The names in dispatch order.
    fn kinds(&self) -> Vec<&'static str> {
        self.log.borrow().iter().map(|(n, _)| *n).collect()
    }

    fn detail(&self, i: usize) -> EventDetail {
        self.log.borrow()[i].1.clone()
    }
}

fn doc_64() -> Document {
    new_document(NewDocumentOptions::default().size(64, 64)).expect("valid")
}

fn session_32() -> Session {
    let mut s = Session::new();
    s.new_document(NewDocumentOptions::default().size(32, 32))
        .expect("new");
    s
}

// <META - ROLE : B5.1 document invariants | L17-45>
#[path = "model_basic/cases_01.rs"]
mod cases_01;
#[path = "model_basic/cases_02.rs"]
mod cases_02;
