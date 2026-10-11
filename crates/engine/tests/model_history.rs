//! `<META - FILE SUMMARY - B5.4 history + B5.6 mergeDown, ported from tests/model_history.test.mjs>`
//!
//! The undo-semantics gate. Undo is where a port most easily goes subtly wrong,
//! so every case here pins one of the JS tree's decisions: the merge policy, the
//! byte budget, the identity-based dirty flag, and the fact that a failing
//! command leaves the stacks untouched.

use pixel_engine::blend::over;
use pixel_engine::errors::DrawToolError;
use pixel_engine::model::document::Document;
use pixel_engine::model::history::{HistoryNotify, NotifyKind};
use pixel_engine::model::{
    Command, CommandEffects, Layer, LayerField, LayerPropValue, MergeDownCommand,
    NewDocumentOptions, Session, SetLayerPropCommand, UndoManager, new_document,
};
use pixel_engine::pixel::{pack_rgba, unpack_rgba};
use std::cell::RefCell;
use std::rc::Rc;

const WHITE: u32 = 0xffffffff;
const BLACK: u32 = 0xff000000;

/// A command whose `do` fails, like the JS test's inline `failing` object.
#[derive(Debug)]
struct FailingDo;

impl Command for FailingDo {
    fn label(&self) -> &str {
        "fail"
    }

    fn type_name(&self) -> &'static str {
        "x"
    }

    fn apply(&mut self, _doc: &mut Document) -> Result<(), DrawToolError> {
        Err(DrawToolError::invalid_state("do-fail"))
    }

    fn revert(&mut self, _doc: &mut Document) -> Result<(), DrawToolError> {
        Ok(())
    }

    fn byte_size(&self) -> usize {
        0
    }

    fn effects(&self) -> CommandEffects {
        CommandEffects::default()
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `newDocument({widthPx: 32, heightPx: 32})` + `new UndoManager({})`.
fn opacity_doc() -> (Document, UndoManager) {
    let doc = new_document(NewDocumentOptions::default().size(32, 32)).expect("valid");
    let mut hist = UndoManager::new();
    hist.set_document();
    (doc, hist)
}

fn name_cmd(id: &str, before: &str, after: &str, label: &str) -> SetLayerPropCommand {
    SetLayerPropCommand::with_label(
        id,
        LayerField::Name,
        LayerPropValue::Name(before.into()),
        LayerPropValue::Name(after.into()),
        label,
    )
}

fn opacity_cmd(id: &str, from: f64, to: f64) -> SetLayerPropCommand {
    SetLayerPropCommand::with_label(
        id,
        LayerField::Opacity,
        LayerPropValue::Opacity(from),
        LayerPropValue::Opacity(to),
        "o",
    )
}

// <META - ROLE : B5.4 merges 30 opacity commits into one entry | L20-30>
#[test]
fn thirty_opacity_commits_collapse_into_one_entry() {
    let (mut doc, mut hist) = opacity_doc();
    let id = doc.active_layer_id.clone();
    for i in 0..30 {
        let opacity = f64::from(i + 1) / 30.0;
        let current = doc.get_layer(&id).expect("layer").opacity;
        hist.apply(&mut doc, opacity_cmd(&id, current, opacity))
            .expect("commit");
    }
    let mut n = 0;
    while hist.undo(&mut doc).expect("undo") {
        n += 1;
    }
    assert_eq!(n, 1);
    assert_eq!(doc.get_layer(&id).expect("layer").opacity, 1.0);
}

// <META - ROLE : B5.4 breakMerge starts a new entry | L32-44>
#[test]
fn break_merge_starts_a_new_entry() {
    let (mut doc, mut hist) = opacity_doc();
    let id = doc.active_layer_id.clone();
    hist.apply(&mut doc, opacity_cmd(&id, 1.0, 0.5))
        .expect("commit");
    hist.break_merge();
    hist.apply(&mut doc, opacity_cmd(&id, 0.5, 0.2))
        .expect("commit");
    assert_eq!(hist.undo_label().as_deref(), Some("o"));
    assert!(hist.undo(&mut doc).expect("undo"));
    assert_eq!(doc.get_layer(&id).expect("layer").opacity, 0.5);
    assert!(hist.undo(&mut doc).expect("undo"));
    assert_eq!(doc.get_layer(&id).expect("layer").opacity, 1.0);
    assert!(!hist.can_undo());
}

// <META - ROLE : B5.4 evicts oldest under budget but keeps one | L46-56>
#[test]
fn evicts_the_oldest_but_always_keeps_one() {
    let mut doc = new_document(NewDocumentOptions::default().size(32, 32)).expect("valid");
    let id = doc.active_layer_id.clone();
    let mut hist = UndoManager::with_limits(3, pixel_engine::constants::UNDO_MAX_BYTES);
    hist.set_document();
    for i in 0..5 {
        hist.apply(
            &mut doc,
            name_cmd(
                &id,
                &format!("n{i}"),
                &format!("n{}", i + 1),
                &format!("c{i}"),
            ),
        )
        .expect("commit");
    }
    let mut n = 0;
    while hist.undo(&mut doc).expect("undo") {
        n += 1;
    }
    assert_eq!(n, 3);
}

// <META - ROLE : B5.4 dirty/savedLost semantics | L58-72>
#[test]
fn dirty_and_saved_lost_semantics() {
    let mut doc = new_document(NewDocumentOptions::default().size(32, 32)).expect("valid");
    let id = doc.active_layer_id.clone();
    let mut hist = UndoManager::with_limits(2, pixel_engine::constants::UNDO_MAX_BYTES);
    hist.set_document();
    assert!(!hist.is_dirty());
    hist.apply(&mut doc, name_cmd(&id, "a", "b", "c0"))
        .expect("commit");
    assert!(hist.is_dirty());
    hist.mark_saved();
    assert!(!hist.is_dirty());
    hist.apply(&mut doc, name_cmd(&id, "b", "c", "c1"))
        .expect("commit");
    hist.apply(&mut doc, name_cmd(&id, "c", "d", "c2"))
        .expect("commit");
    assert!(hist.is_dirty(), "the evicted entry was the saved one");
    hist.mark_saved();
    assert!(!hist.is_dirty());
}

// <META - ROLE : B5.4 commit clears redo and notifies in order, tolerating failures | L74-98>
#[test]
fn commit_notifies_in_order_and_tolerates_failures() {
    let (mut doc, mut hist) = opacity_doc();
    let id = doc.active_layer_id.clone();
    let order = Rc::new(RefCell::new(Vec::<String>::new()));
    let a = Rc::clone(&order);
    hist.subscribe(move |n: &HistoryNotify<'_>| {
        a.borrow_mut().push(format!("first:{}", n.kind.as_str()));
        Ok(())
    });
    hist.subscribe(|_| Err(DrawToolError::invalid_state("boom")));
    let c = Rc::clone(&order);
    hist.subscribe(move |n: &HistoryNotify<'_>| {
        c.borrow_mut().push(format!("third:{}", n.kind.as_str()));
        Ok(())
    });
    hist.apply(&mut doc, name_cmd(&id, "a", "b", "c"))
        .expect("commit");
    assert_eq!(&*order.borrow(), &["first:commit", "third:commit"]);
    assert!(hist.undo(&mut doc).expect("undo"));
    assert_eq!(order.borrow()[2..].to_vec(), ["first:undo", "third:undo"]);
    assert!(hist.can_redo());
    hist.apply(&mut doc, name_cmd(&id, "b", "c", "c2"))
        .expect("commit");
    assert!(!hist.can_redo(), "a commit clears the redo stack");
}

// <META - ROLE : B5.4 a failing command object does not mutate the stack | L100-122>
#[test]
fn a_failing_command_does_not_mutate_the_stack() {
    let (mut doc, mut hist) = opacity_doc();
    let id = doc.active_layer_id.clone();
    hist.apply(&mut doc, name_cmd(&id, "a", "b", "ok"))
        .expect("commit");
    let err = hist.apply(&mut doc, FailingDo).expect_err("do-fail");
    assert_eq!(err.code, pixel_engine::errors::CODE_INVALID_STATE);
    assert_eq!(hist.undo_label().as_deref(), Some("ok"));
    assert!(!hist.can_redo());
}

/// `NotifyKind` is the only other thing the listener signature exposes; this
/// keeps the import honest and pins the four strings.
#[test]
fn notify_kind_strings_are_the_js_ones() {
    assert_eq!(NotifyKind::Commit.as_str(), "commit");
    assert_eq!(NotifyKind::Undo.as_str(), "undo");
    assert_eq!(NotifyKind::Redo.as_str(), "redo");
    assert_eq!(NotifyKind::Clear.as_str(), "clear");
}

// <META - ROLE : B5.6 matches golden composite cases and round-trips undo | L126-150>
#[test]
fn merge_down_matches_the_composite_and_round_trips() {
    let mut session = Session::new();
    session
        .new_document(NewDocumentOptions::default().size(32, 32))
        .expect("new");
    let lower_id = session.doc().expect("doc").active_layer_id.clone();
    let doc = session.doc_mut().expect("doc");
    doc.get_layer_mut(&lower_id)
        .expect("layer")
        .store
        .set_pixel(0, 0, BLACK);
    doc.get_layer_mut(&lower_id).expect("layer").opacity = 1.0;
    let upper_id = session.add_layer(Some("upper")).expect("add");
    let doc = session.doc_mut().expect("doc");
    doc.get_layer_mut(&upper_id)
        .expect("layer")
        .store
        .set_pixel(0, 0, WHITE);
    doc.get_layer_mut(&upper_id).expect("layer").opacity = 1.0;
    session.merge_down(&upper_id).expect("merge");

    let got = unpack_rgba(
        session
            .doc()
            .expect("doc")
            .get_layer(&lower_id)
            .expect("layer")
            .store
            .get_pixel(0, 0),
    );
    assert_eq!(
        got,
        over(
            over([0, 0, 0, 0], [0, 0, 0, 255], 1.0),
            [255, 255, 255, 255],
            1.0
        )
    );
    assert_eq!(session.doc().expect("doc").layers.len(), 1);

    assert!(session.undo().expect("undo"));
    let doc = session.doc().expect("doc");
    assert_eq!(doc.layers.len(), 2);
    assert_eq!(
        unpack_rgba(
            doc.get_layer(&lower_id)
                .expect("layer")
                .store
                .get_pixel(0, 0)
        ),
        [0, 0, 0, 255]
    );
    assert_eq!(
        unpack_rgba(
            doc.get_layer(&upper_id)
                .expect("layer")
                .store
                .get_pixel(0, 0)
        ),
        [255, 255, 255, 255]
    );
    assert!(session.redo().expect("redo"));
    assert_eq!(session.doc().expect("doc").layers.len(), 1);
}

// <META - ROLE : B5.6 bakes opacity (golden white@0.5 over black) and sets opacity to 1 | L152-169>
#[test]
fn merge_down_bakes_opacity_and_sets_it_to_one() {
    let mut session = Session::new();
    session
        .new_document(NewDocumentOptions::default().size(32, 32))
        .expect("new");
    let lower_id = session.doc().expect("doc").active_layer_id.clone();
    session
        .doc_mut()
        .expect("doc")
        .get_layer_mut(&lower_id)
        .expect("layer")
        .store
        .set_pixel(3, 3, BLACK);
    let upper_id = session.add_layer(Some("upper")).expect("add");
    let doc = session.doc_mut().expect("doc");
    doc.get_layer_mut(&upper_id)
        .expect("layer")
        .store
        .set_pixel(3, 3, WHITE);
    doc.get_layer_mut(&upper_id).expect("layer").opacity = 0.5;
    session.merge_down(&upper_id).expect("merge");

    // Golden: white@0.5 over black is [128,128,128,255].
    let got = unpack_rgba(
        session
            .doc()
            .expect("doc")
            .get_layer(&lower_id)
            .expect("layer")
            .store
            .get_pixel(3, 3),
    );
    assert_eq!(got, [128, 128, 128, 255]);
    let doc = session.doc().expect("doc");
    assert_eq!(doc.get_layer(&lower_id).expect("layer").opacity, 1.0);
    assert_eq!(doc.active_layer_id, lower_id);
    assert!(session.undo().expect("undo"));
    let doc = session.doc().expect("doc");
    assert_eq!(doc.get_layer(&lower_id).expect("layer").opacity, 1.0);
    assert_eq!(doc.get_layer(&upper_id).expect("layer").opacity, 0.5);
}

// <META - ROLE : B5.6 rejects locked/hidden and the bottom layer | L171-182>
#[test]
fn merge_down_rejects_locked_hidden_and_the_bottom_layer() {
    let mut session = Session::new();
    session
        .new_document(NewDocumentOptions::default().size(32, 32))
        .expect("new");
    let bottom = session.doc().expect("doc").active_layer_id.clone();
    let top = session.add_layer(Some("top")).expect("add");
    let err = session.merge_down(&bottom).expect_err("no layer below");
    assert_eq!(err.code, pixel_engine::errors::CODE_INVALID_STATE);

    session.set_layer_locked(&top, true).expect("lock");
    let err = session.merge_down(&top).expect_err("locked");
    assert_eq!(err.code, pixel_engine::errors::CODE_LAYER_LOCKED);
    session.set_layer_locked(&top, false).expect("unlock");

    session.set_layer_visible(&bottom, false).expect("hide");
    let before = session.history().state();
    let err = session.merge_down(&top).expect_err("hidden");
    assert_eq!(err.code, pixel_engine::errors::CODE_LAYER_HIDDEN);
    // Nothing moved and no history entry was added.
    assert_eq!(session.doc().expect("doc").layers.len(), 2);
    assert_eq!(session.history().state(), before);
}

// <META - ROLE : B5.6 the direct MergeDownCommand round-trips bytes | L184-199>
#[test]
fn direct_merge_down_command_round_trips_bytes() {
    let mut doc = new_document(NewDocumentOptions::default().size(32, 32)).expect("valid");
    let lower_id = doc.active_layer_id.clone();
    doc.get_layer_mut(&lower_id)
        .expect("layer")
        .store
        .set_pixel(1, 1, pack_rgba(255.0, 0.0, 0.0, 255.0));
    let mut upper = Layer::create(doc.ids.next(), "U", 32, 32).expect("valid");
    let upper_id = upper.id.clone();
    upper
        .store
        .set_pixel(1, 1, pack_rgba(0.0, 0.0, 255.0, 255.0));
    doc.insert(upper, 1).expect("insert");

    let mut cmd = MergeDownCommand::new(upper_id.as_str());
    Command::apply(&mut cmd, &mut doc).expect("do");
    assert_eq!(
        unpack_rgba(
            doc.get_layer(&lower_id)
                .expect("layer")
                .store
                .get_pixel(1, 1)
        ),
        [0, 0, 255, 255]
    );
    Command::revert(&mut cmd, &mut doc).expect("undo");
    assert_eq!(
        unpack_rgba(
            doc.get_layer(&lower_id)
                .expect("layer")
                .store
                .get_pixel(1, 1)
        ),
        [255, 0, 0, 255]
    );
    assert_eq!(
        unpack_rgba(
            doc.get_layer(&upper_id)
                .expect("layer")
                .store
                .get_pixel(1, 1)
        ),
        [0, 0, 255, 255]
    );
    Command::apply(&mut cmd, &mut doc).expect("do again");
    assert_eq!(
        unpack_rgba(
            doc.get_layer(&lower_id)
                .expect("layer")
                .store
                .get_pixel(1, 1)
        ),
        [0, 0, 255, 255]
    );
}
