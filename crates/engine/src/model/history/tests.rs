use super::*;
use crate::constants::{UNDO_MAX_BYTES, UNDO_MAX_STEPS};
use crate::errors::{DrawToolError, EngineResult};
use crate::model::command::{Command, CommandEffects};
use crate::model::commands::SetLayerPropCommand;
use crate::model::document::{Document, NewDocumentOptions, new_document};
use crate::model::layer::{LayerField, LayerPropValue};

/// A command that fails on `do`, for the failure-isolation test.
#[derive(Debug)]
struct FailingCommand;

impl Command for FailingCommand {
    fn label(&self) -> &str {
        "fail"
    }

    fn type_name(&self) -> &'static str {
        "x"
    }

    fn apply(&mut self, _doc: &mut Document) -> EngineResult<()> {
        Err(DrawToolError::invalid_state("do-fail"))
    }

    fn revert(&mut self, _doc: &mut Document) -> EngineResult<()> {
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

fn doc() -> Document {
    new_document(NewDocumentOptions::default().size(32, 32)).expect("valid")
}

fn name_cmd(id: &str, before: &str, after: &str, label: &str) -> SetLayerPropCommand {
    SetLayerPropCommand::with_label(
        id.to_string(),
        LayerField::Name,
        LayerPropValue::Name(before.to_string()),
        LayerPropValue::Name(after.to_string()),
        label,
    )
}

// <META - ROLE : parity with tests/model_history.test.mjs B5.4 merge case | L20-30>
#[test]
fn thirty_opacity_commits_collapse_into_one_entry() {
    let mut d = doc();
    let id = d.active_layer_id.clone();
    let mut hist = UndoManager::new();
    hist.set_document();
    for i in 0..30 {
        let opacity = f64::from(i + 1) / 30.0;
        let current = d.get_layer(&id).unwrap().opacity;
        hist.apply(
            &mut d,
            SetLayerPropCommand::with_label(
                id.clone(),
                LayerField::Opacity,
                LayerPropValue::Opacity(current),
                LayerPropValue::Opacity(opacity),
                "o",
            ),
        )
        .expect("commit");
    }
    let mut n = 0;
    while hist.undo(&mut d).expect("undo") {
        n += 1;
    }
    assert_eq!(n, 1);
    assert_eq!(d.get_layer(&id).unwrap().opacity, 1.0);
}

#[test]
fn break_merge_starts_a_new_entry() {
    let mut d = doc();
    let id = d.active_layer_id.clone();
    let mut hist = UndoManager::new();
    hist.set_document();
    hist.apply(
        &mut d,
        SetLayerPropCommand::with_label(
            id.clone(),
            LayerField::Opacity,
            LayerPropValue::Opacity(1.0),
            LayerPropValue::Opacity(0.5),
            "o",
        ),
    )
    .expect("commit");
    hist.break_merge();
    hist.apply(
        &mut d,
        SetLayerPropCommand::with_label(
            id.clone(),
            LayerField::Opacity,
            LayerPropValue::Opacity(0.5),
            LayerPropValue::Opacity(0.2),
            "o",
        ),
    )
    .expect("commit");
    assert_eq!(hist.undo_label().as_deref(), Some("o"));
    assert!(hist.undo(&mut d).expect("undo"));
    assert_eq!(d.get_layer(&id).unwrap().opacity, 0.5);
    assert!(hist.undo(&mut d).expect("undo"));
    assert_eq!(d.get_layer(&id).unwrap().opacity, 1.0);
    assert!(!hist.can_undo());
}

// <META - ROLE : parity with tests/model_history.test.mjs B5.4 eviction case | L46-56>
#[test]
fn evicts_the_oldest_but_always_keeps_one() {
    let mut d = doc();
    let id = d.active_layer_id.clone();
    let mut hist = UndoManager::with_limits(3, UNDO_MAX_BYTES);
    hist.set_document();
    for i in 0..5 {
        hist.apply(
            &mut d,
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
    while hist.undo(&mut d).expect("undo") {
        n += 1;
    }
    assert_eq!(n, 3);
}

// <META - ROLE : parity with tests/model_history.test.mjs B5.4 dirty case | L58-72>
#[test]
fn dirty_and_saved_lost_semantics() {
    let mut d = doc();
    let id = d.active_layer_id.clone();
    let mut hist = UndoManager::with_limits(2, UNDO_MAX_BYTES);
    hist.set_document();
    assert!(!hist.is_dirty());
    hist.mark_saved();
    assert!(!hist.is_dirty());
    hist.apply(&mut d, name_cmd(&id, "a", "b", "c0"))
        .expect("commit");
    assert!(hist.is_dirty());
    hist.mark_saved();
    assert!(!hist.is_dirty());
    hist.apply(&mut d, name_cmd(&id, "b", "c", "c1"))
        .expect("commit");
    hist.apply(&mut d, name_cmd(&id, "c", "d", "c2"))
        .expect("commit");
    assert!(hist.is_dirty(), "the evicted entry was the saved one");
    hist.mark_saved();
    assert!(!hist.is_dirty());
}

#[test]
fn a_failing_do_does_not_mutate_the_stack() {
    let mut d = doc();
    let id = d.active_layer_id.clone();
    let mut hist = UndoManager::new();
    hist.set_document();
    hist.apply(&mut d, name_cmd(&id, "a", "b", "ok"))
        .expect("commit");
    let err = hist.apply(&mut d, FailingCommand).expect_err("must fail");
    assert_eq!(err.code, crate::errors::CODE_INVALID_STATE);
    assert_eq!(hist.undo_label().as_deref(), Some("ok"));
    assert!(!hist.can_redo());
}

// <META - ROLE : parity with tests/model_history.test.mjs B5.4 order case | L74-98>
#[test]
fn commit_notifies_in_order_and_tolerates_failures() {
    let mut d = doc();
    let id = d.active_layer_id.clone();
    let mut hist = UndoManager::new();
    hist.set_document();
    let order = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let a = order.clone();
    hist.subscribe(move |n| {
        a.borrow_mut().push(format!("first:{}", n.kind.as_str()));
        Ok(())
    });
    hist.subscribe(|_| Err(DrawToolError::invalid_state("boom")));
    let c = order.clone();
    hist.subscribe(move |n| {
        c.borrow_mut().push(format!("third:{}", n.kind.as_str()));
        Ok(())
    });
    hist.apply(&mut d, name_cmd(&id, "a", "b", "c"))
        .expect("commit");
    assert_eq!(&*order.borrow(), &["first:commit", "third:commit"]);
    assert!(hist.undo(&mut d).expect("undo"));
    assert_eq!(order.borrow()[2..].to_vec(), &["first:undo", "third:undo"]);
    assert!(hist.can_redo());
    hist.apply(&mut d, name_cmd(&id, "b", "c", "c2"))
        .expect("commit");
    assert!(!hist.can_redo(), "a commit clears the redo stack");
}

#[test]
fn clear_resets_everything_and_notifies() {
    let mut d = doc();
    let id = d.active_layer_id.clone();
    let mut hist = UndoManager::new();
    hist.set_document();
    hist.mark_saved();
    hist.apply(&mut d, name_cmd(&id, "a", "b", "c"))
        .expect("commit");
    assert!(hist.is_dirty());
    let seen = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let inner = std::rc::Rc::clone(&seen);
    let sub = hist.subscribe(move |n| {
        if n.kind == NotifyKind::Clear {
            inner.set(inner.get() + 1);
        }
        Ok(())
    });
    hist.clear();
    assert_eq!(seen.get(), 1);
    assert!(!hist.can_undo());
    assert!(!hist.can_redo());
    assert!(!hist.is_dirty());
    assert_eq!(hist.total_bytes, 0);
    // `last_effects` survives a clear, matching the JS subscription skip.
    assert!(hist.last_effects().is_some());
    hist.unsubscribe(sub);
    hist.clear();
    // The listener is gone, so `seen` stays at 1.
    assert_eq!(seen.get(), 1);
}

#[test]
fn undo_of_nothing_is_false_not_an_error() {
    let mut d = doc();
    let mut hist = UndoManager::new();
    assert!(!hist.undo(&mut d).expect("undo"));
    assert!(!hist.redo(&mut d).expect("redo"));
    assert_eq!(hist.undo_label(), None);
    assert_eq!(hist.state(), HistoryState::default());
}

#[test]
fn defaults_are_the_contract_numbers() {
    let h = UndoManager::new();
    assert_eq!(h.limit_steps, UNDO_MAX_STEPS);
    assert_eq!(h.limit_bytes, UNDO_MAX_BYTES);
}
