//! Autosave scheduling and restore-record regression tests.

use super::*;
use pixel_engine::model::{EditOptions, NewDocumentOptions};
use pixel_engine::pixel::pack_rgba_bytes;
use serde_json::json;

/// One temp autosave file, removed on drop.
struct TempFile(PathBuf);
impl TempFile {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pixeldraw-autosave-{tag}-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        Self(path)
    }
    fn read_json(&self) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(&self.0).expect("written"))
            .expect("valid json")
    }
}
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The default 512×512 doc with a second layer, a pixel, and 50 % opacity.
fn painted_session() -> Session {
    let mut session = Session::new();
    session
        .new_document(NewDocumentOptions::default())
        .expect("default doc");
    session.add_layer(None).expect("second layer");
    session
        .stroke_begin(EditOptions::on_active_layer().labeled("test dot"))
        .expect("stroke");
    session
        .stroke_with(|writer| {
            writer.set(3, 4, pack_rgba_bytes(255, 0, 0, 255));
        })
        .expect("paint");
    session.stroke_commit(Some("test dot")).expect("commit");
    let id = session.doc().expect("doc").active_layer_id.clone();
    session.set_layer_opacity(&id, 0.5, true).expect("opacity");
    session
}

#[test]
fn record_round_trips_layers_pixels_and_active_layer() {
    let session = painted_session();
    let doc = session.doc().expect("doc");
    let record = AutosaveRecord::build(doc, 42).expect("build");
    let bytes = record.encode();

    let parsed = AutosaveRecord::parse(&bytes).expect("parses");
    assert_eq!(parsed.updated_at_ms, 42);
    assert_eq!(parsed.name(), "Untitled");
    assert_eq!(parsed.when(), format_timestamp(42));
    let restored = parsed.into_document().expect("loads");
    assert_eq!(restored.layers.len(), 2);
    assert_eq!(
        restored.active_layer_id, doc.active_layer_id,
        "active layer survives"
    );
    let active = restored
        .layers
        .iter()
        .find(|l| l.id == restored.active_layer_id)
        .expect("active layer present");
    assert_eq!(active.opacity, 0.5);
    // The pixel reaches the flat image at (3, 4).
    let flat = pixel_engine::io::export::flatten_to_rgba(
        &restored.layer_sources(),
        restored.canvas.width_px,
        restored.canvas.height_px,
        pixel_engine::io::export::FlattenOptions {
            include_background: true,
        },
    );
    let o = ((4 * restored.canvas.width_px) + 3) as usize * 4;
    assert_eq!(&flat[o..o + 4], &[255, 0, 0, 128], "50 % red dot");
}

#[test]
fn parse_rejects_structural_damage() {
    let session = painted_session();
    let record = AutosaveRecord::build(session.doc().expect("doc"), 1).expect("build");
    let base: serde_json::Value = serde_json::from_slice(&record.encode()).expect("json");
    let encode = |v: &serde_json::Value| serde_json::to_vec(v).expect("json");

    let mut v = base.clone();
    v["schema"] = json!(1);
    assert!(AutosaveRecord::parse(&encode(&v)).is_none(), "schema != 2");

    let mut v = base.clone();
    v["document"]["document_id"] = json!("");
    assert!(AutosaveRecord::parse(&encode(&v)).is_none(), "empty id");

    let mut v = base.clone();
    v["document"]["canvas"]["width_px"] = json!(48);
    assert!(
        AutosaveRecord::parse(&encode(&v)).is_none(),
        "48 px is not a valid canvas width"
    );

    let mut v = base.clone();
    v["document"]["layers"] = json!([]);
    assert!(AutosaveRecord::parse(&encode(&v)).is_none(), "no layers");

    let mut v = base.clone();
    v["document"]["layers"] = json!((0..65).map(|_| json!({})).collect::<Vec<_>>());
    assert!(AutosaveRecord::parse(&encode(&v)).is_none(), "65 layers");

    // Structurally plausible but deep-invalid: parse passes, load fails.
    let mut v = base.clone();
    v["document"]["layers"][0]["opacity"] = json!(1.5);
    let rec = AutosaveRecord::parse(&encode(&v)).expect("structural ok");
    assert!(rec.into_document().is_err(), "opacity > 1 rejected");

    let mut v = base.clone();
    let layer_with_chunks = v["document"]["layers"]
        .as_array()
        .unwrap()
        .iter()
        .position(|l| {
            l["raster"]["chunks"]
                .as_array()
                .is_some_and(|c| !c.is_empty())
        })
        .expect("the painted layer has chunks");
    v["document"]["layers"][layer_with_chunks]["raster"]["chunks"][0]["png"] = json!("!!!");
    let rec = AutosaveRecord::parse(&encode(&v)).expect("structural ok");
    assert!(rec.into_document().is_err(), "corrupt chunk rejected");
}

#[test]
fn timestamps_format_as_fixed_utc() {
    assert_eq!(format_timestamp(0), "1970-01-01 00:00:00");
    assert_eq!(format_timestamp(90_061_000), "1970-01-02 01:01:01");
    assert_eq!(format_timestamp(1_750_000_000_000), "2025-06-15 15:06:40");
}

#[test]
fn debounce_does_not_reset_on_new_events() {
    let base = Instant::now();
    let mut scheduler = Scheduler::default();
    scheduler.mark(base);
    scheduler.mark(base + Duration::from_millis(700));
    assert!(!scheduler.due(base + Duration::from_millis(799)));
    assert!(
        scheduler.due(base + DEBOUNCE),
        "flushes 800 ms after the FIRST event"
    );
    assert!(DEBOUNCE < MAX_DELAY, "the hard cap only ever backstops");
}

#[test]
fn failed_flush_needs_a_new_event_to_reschedule() {
    let base = Instant::now();
    let mut scheduler = Scheduler::default();
    scheduler.mark(base);
    scheduler.failed();
    assert!(!scheduler.due(base + Duration::from_secs(10)));
    assert!(scheduler.is_dirty(), "data stays dirty for the retry");
    scheduler.mark(base + Duration::from_secs(11));
    assert!(scheduler.due(base + Duration::from_secs(11) + DEBOUNCE));
}

#[test]
fn boot_offers_a_valid_record_and_deletes_a_broken_one() {
    let file = TempFile::new("boot");
    let session = painted_session();
    let bytes = AutosaveRecord::build(session.doc().expect("doc"), 7)
        .expect("build")
        .encode();
    std::fs::write(&file.0, &bytes).expect("seed");
    let glue = AutosaveGlue::boot_from(Some(file.0.clone()));
    let (name, when) = glue.peek().expect("offered");
    assert_eq!(name, "Untitled");
    assert_eq!(when, format_timestamp(7));

    let broken = TempFile::new("boot-broken");
    std::fs::write(&broken.0, b"{\"schema\": 1}").expect("seed");
    let glue = AutosaveGlue::boot_from(Some(broken.0.clone()));
    assert!(glue.peek().is_none());
    assert!(!broken.0.exists(), "invalid file is cleared");
}

#[test]
fn restore_and_discard_resolve_the_boot_record() {
    let file = TempFile::new("restore");
    let session = painted_session();
    let bytes = AutosaveRecord::build(session.doc().expect("doc"), 7)
        .expect("build")
        .encode();
    std::fs::write(&file.0, &bytes).expect("seed");

    let mut glue = AutosaveGlue::boot_from(Some(file.0.clone()));
    let doc = glue.restore_pending().expect("a record").expect("loads");
    assert_eq!(doc.layers.len(), 2);
    assert!(glue.peek().is_none(), "the record is consumed");
    assert!(file.0.exists(), "restore keeps the file");

    let mut glue = AutosaveGlue::boot_from(Some(file.0.clone()));
    glue.clear();
    assert!(glue.peek().is_none());
    assert!(!file.0.exists(), "discard deletes the file");
}

#[test]
fn flush_writes_when_dirty_and_is_idempotent() {
    let file = TempFile::new("flush");
    let session = painted_session();
    let mut glue = AutosaveGlue::boot_from(Some(file.0.clone()));
    assert!(
        glue.flush_now(&session).is_none(),
        "nothing dirty, nothing to do"
    );

    let base = Instant::now();
    glue.on_event(&ModelEvent::pixels_all_changed("layer-0001"), base);
    assert!(
        glue.flush_if_due(&session, base + Duration::from_millis(100))
            .is_none(),
        "before the debounce"
    );
    let outcome = glue.flush_if_due(&session, base + DEBOUNCE).expect("due");
    assert!(outcome.is_ok());
    let root = file.read_json();
    assert_eq!(root["schema"], json!(2));
    assert_eq!(root["format"], json!(FORMAT));
    assert_eq!(root["document"]["format"], json!("draw_tool.document"));
    assert!(
        glue.flush_if_due(&session, base + Duration::from_secs(1))
            .is_none(),
        "clean after a successful flush"
    );
}

#[test]
fn events_that_do_not_touch_the_document_are_ignored() {
    let now = Instant::now();
    let mut glue = AutosaveGlue::boot_from(None);
    glue.on_event(
        &ModelEvent::settings_changed(
            pixel_engine::model::SettingKey::PenSize,
            pixel_engine::model::SettingValue::Int(2),
        ),
        now,
    );
    assert!(
        !glue.scheduler.is_dirty(),
        "settings alone are not persisted"
    );
    glue.on_event(
        &ModelEvent::history_changed(pixel_engine::model::HistoryState::default()),
        now,
    );
    assert!(!glue.scheduler.is_dirty(), "history alone is not persisted");
    glue.on_event(&ModelEvent::document_replaced(), now);
    assert!(glue.scheduler.is_dirty(), "document replacement counts");
}
