//! W5 acceptance tests — pen / eraser / fill / eyedropper / hand + manager.
//!
//! A real engine `Session` under every tool: strokes must paint, undo must
//! restore, and the JS-observable notifications must fire. No egui, no window.
//! Facade: shared fixtures here, one module per tool beside it.

mod eyedropper;
mod fill;
mod manager;
mod pen;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use pixel_engine::model::{ModelEvent, NewDocumentOptions, Session};
use pixel_engine::view::View;

use crate::input::RawPointer;
use crate::tools::fill::FillTool;
use crate::tools::manager::ToolManager;
use crate::tools::pen::PenTool;

type Queue = Rc<RefCell<VecDeque<ModelEvent>>>;

pub(crate) fn session64() -> (Session, Queue) {
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

pub(crate) fn view1() -> View {
    View::new(1.0, 0.0, 0.0).expect("valid view")
}

pub(crate) fn down(x: f64, y: f64) -> RawPointer {
    RawPointer::new((x, y))
}

pub(crate) fn pixel(session: &Session, x: u32, y: u32) -> u32 {
    let doc = session.doc().expect("doc");
    let layer = doc.get_layer(&doc.active_layer_id).expect("active layer");
    layer.store.get_pixel(x, y)
}

fn active_id(session: &Session) -> String {
    session.doc().expect("doc").active_layer_id.clone()
}

fn manager_pen_fill() -> ToolManager {
    let mut manager = ToolManager::new();
    manager.register(Box::new(PenTool::pen()));
    manager.register(Box::new(FillTool::new()));
    manager.set_initial_id("pen");
    manager
}
