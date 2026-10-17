//! Autosave — `src/io/store_idb.js` collapsed to one file (Phase 3.10).
//!
//! The JS IndexedDB store (a `meta` record + 4096-byte chunk records) becomes a
//! single `autosave.json` holding the engine's document JSON
//! (`io::serialize::document_to_json`): the chunks ride along as PNG-base64 in
//! the same shape a saved `.draw.json` uses, so loading inherits the whole
//! validated path (`validate_document` + `json_to_document`). Whole-file
//! rewrites replace the JS's incremental chunk updates; the schedule is
//! preserved: 800 ms debounce that does **not** reset on new events, a 5000 ms
//! hard cap, and the two failure notices (`자동저장 공간 부족`/`STORAGE_QUOTA`,
//! `자동저장 실패`/`INVALID_STATE`).
//!
//! Timing is injected (`Instant`); the frame loop drives
//! [`AutosaveGlue::flush_if_due`] and flushes eagerly on window close / focus
//! loss (`pagehide` + `visibilitychange` parity).

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pixel_engine::constants::{MAX_LAYERS, is_valid_canvas_size};
use pixel_engine::errors::EngineResult;
use pixel_engine::io::serialize::{document_to_json, json_to_document};
use pixel_engine::model::{Document, EventDetail, ModelEvent, Session};
use pixel_platform::PlatformError;

/// The wrapper's own format tag (this file is not a `.draw.json`).
const FORMAT: &str = "pixeldraw.autosave";
/// JS autosave meta `schema: 2` (IndexedDB `draw_tool_v2` record version).
const SCHEMA: u64 = 2;

/// JS scheduler debounce: 800 ms after the first dirty event.
pub const DEBOUNCE: Duration = Duration::from_millis(800);
/// JS scheduler hard cap: a dirty store flushes by 5 s.
pub const MAX_DELAY: Duration = Duration::from_millis(5000);

/// One autosave snapshot: the document plus its wall-clock stamp.
#[derive(Debug, Clone)]
pub struct AutosaveRecord {
    pub updated_at_ms: u64,
    /// The `draw_tool.document` JSON (`document_to_json`'s output).
    pub document: serde_json::Value,
}

impl AutosaveRecord {
    /// `idb_record_builder.js` `buildMetaRecord` + the chunk records, folded
    /// into one document JSON. `updated_at_ms` is `Date.now()` injected.
    pub fn build(doc: &Document, updated_at_ms: u64) -> EngineResult<Self> {
        Ok(Self {
            updated_at_ms,
            document: document_to_json(doc)?,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "format": FORMAT,
            "schema": SCHEMA,
            "updated_at": self.updated_at_ms,
            "document": self.document,
        }))
        .unwrap_or_default()
    }

    /// Structural validation (`meta_valid`): schema, non-empty document id, a
    /// valid canvas size, 1..=64 layers. Deeper checks (per-layer fields,
    /// chunk decode, document construction) belong to [`Self::into_document`].
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let root: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        if root["format"].as_str() != Some(FORMAT) {
            return None;
        }
        if root["schema"].as_u64() != Some(SCHEMA) {
            return None;
        }
        let updated_at_ms = root["updated_at"].as_u64()?;
        let document = root.get("document")?;
        if !document.is_object() {
            return None;
        }
        let id = document["document_id"].as_str().unwrap_or_default();
        if id.is_empty() {
            return None;
        }
        let w = document["canvas"]["width_px"].as_u64().unwrap_or(0) as u32;
        let h = document["canvas"]["height_px"].as_u64().unwrap_or(0) as u32;
        if !is_valid_canvas_size(w, h) {
            return None;
        }
        let layers = document["layers"].as_array()?;
        if layers.is_empty() || layers.len() > MAX_LAYERS {
            return None;
        }
        Some(Self {
            updated_at_ms,
            document: document.clone(),
        })
    }

    /// The document name shown by the restore dialog (`meta.name`).
    pub fn name(&self) -> String {
        self.document
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string()
    }

    /// `updatedAt` for the restore dialog. `std` has no local timezone, so the
    /// stamp is fixed UTC `YYYY-MM-DD HH:MM:SS` (the JS used the locale string;
    /// the format difference is logged in the progress log).
    pub fn when(&self) -> String {
        format_timestamp(self.updated_at_ms)
    }

    /// Full validation + construction (`store.load()` semantics): the record's
    /// document runs through `validate_document`/`json_to_document`, which also
    /// covers the JS fallbacks (missing blend → normal, no active layer → the
    /// top layer, no name → `Untitled`).
    pub fn into_document(self) -> EngineResult<Document> {
        json_to_document(&self.document).map(|(doc, _warnings)| doc)
    }
}

/// The 800 ms/5000 ms schedule (`store_idb.js` `_scheduler`/`_schedule`).
#[derive(Default)]
pub struct Scheduler {
    dirty: bool,
    /// First dirty instant of the current burst; never moved while dirty —
    /// the JS `if (!_timer)` guard, so the debounce does not reset.
    first_dirty: Option<Instant>,
}

impl Scheduler {
    pub fn mark(&mut self, now: Instant) {
        self.dirty = true;
        if self.first_dirty.is_none() {
            self.first_dirty = Some(now);
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Due 800 ms after the first dirty event. `MAX_DELAY` is the JS contract's
    /// hard cap; with `mark` never moving `first_dirty` it can only bind if a
    /// future scheduler variant resets the debounce.
    pub fn due(&self, now: Instant) -> bool {
        let Some(first) = self.first_dirty else {
            return false;
        };
        now.saturating_duration_since(first) >= DEBOUNCE
    }

    pub fn clear(&mut self) {
        self.dirty = false;
        self.first_dirty = None;
    }

    /// A failed flush keeps the data dirty but clears the timer: the JS left
    /// `_timer = null` after an attempt, so a retry needs the next event.
    pub fn failed(&mut self) {
        self.first_dirty = None;
    }
}

/// Why a flush failed — the two JS notify branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushError {
    /// `QuotaExceededError` → `자동저장 공간 부족` / `STORAGE_QUOTA`.
    Quota,
    /// Everything else → `자동저장 실패` / `INVALID_STATE`.
    Other,
}

/// Frame-loop glue: boot peek, event tracking, flush.
pub struct AutosaveGlue {
    path: Option<PathBuf>,
    /// Boot record awaiting the restore dialog; consumed by restore/discard.
    pending: Option<AutosaveRecord>,
    scheduler: Scheduler,
}

impl AutosaveGlue {
    /// Boot: read + structurally validate. An invalid file is deleted
    /// (3.10a `load`: "검증 실패 → clear() 후 None").
    pub fn boot() -> Self {
        Self::boot_from(pixel_platform::autosave_file())
    }

    /// No file: nothing read or written (headless tests, non-shell callers).
    pub(crate) fn disabled() -> Self {
        Self::boot_from(None)
    }

    pub(crate) fn boot_from(path: Option<PathBuf>) -> Self {
        let mut pending = None;
        if let Some(p) = &path
            && let Ok(bytes) = std::fs::read(p)
        {
            match AutosaveRecord::parse(&bytes) {
                Some(rec) => pending = Some(rec),
                None => {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
        Self {
            path,
            pending,
            scheduler: Scheduler::default(),
        }
    }

    /// `(name, when)` for the restore dialog; `None` when nothing to offer.
    pub fn peek(&self) -> Option<(String, String)> {
        self.pending.as_ref().map(|r| (r.name(), r.when()))
    }

    /// The user chose 복원: full validation + decode. A record failing the
    /// deep checks is dropped by the caller via [`Self::clear`].
    pub fn restore_pending(&mut self) -> Option<EngineResult<Document>> {
        let rec = self.pending.take()?;
        Some(rec.into_document())
    }

    /// The user chose 버리기 (or Esc / close): delete the file (D8 parity).
    pub fn clear(&mut self) {
        self.pending = None;
        self.scheduler.clear();
        if let Some(p) = &self.path {
            let _ = std::fs::remove_file(p);
        }
    }

    /// Track the three JS subscriptions: pixels, layers, document-replaced.
    pub fn on_event(&mut self, ev: &ModelEvent, now: Instant) {
        match ev.detail {
            EventDetail::PixelsChunks { .. }
            | EventDetail::PixelsAll { .. }
            | EventDetail::Layers { .. }
            | EventDetail::DocumentReplaced => self.scheduler.mark(now),
            _ => {}
        }
    }

    /// Flush when due (called every frame). `None` = nothing to do.
    pub fn flush_if_due(
        &mut self,
        session: &Session,
        now: Instant,
    ) -> Option<Result<(), FlushError>> {
        if !self.scheduler.due(now) {
            return None;
        }
        Some(self.flush(session))
    }

    /// Immediate flush — window close / focus loss (`pagehide` parity).
    pub fn flush_now(&mut self, session: &Session) -> Option<Result<(), FlushError>> {
        if !self.scheduler.is_dirty() {
            return None;
        }
        Some(self.flush(session))
    }

    fn flush(&mut self, session: &Session) -> Result<(), FlushError> {
        let Some(doc) = session.doc() else {
            // JS `flushNow` returns when there is no document.
            self.scheduler.clear();
            return Ok(());
        };
        let record = match AutosaveRecord::build(doc, now_ms()) {
            Ok(record) => record,
            Err(_) => {
                self.scheduler.failed();
                return Err(FlushError::Other);
            }
        };
        let Some(path) = &self.path else {
            self.scheduler.clear();
            return Ok(());
        };
        if pixel_platform::ensure_app_data_dir().is_err() {
            self.scheduler.failed();
            return Err(FlushError::Other);
        }
        match pixel_platform::write_bytes_atomic(path, &record.encode()) {
            Ok(()) => {
                self.scheduler.clear();
                Ok(())
            }
            Err(e) => {
                self.scheduler.failed();
                Err(classify(&e))
            }
        }
    }
}

/// `QuotaExceededError` has no std analogue; disk-full is the nearest.
fn classify(e: &PlatformError) -> FlushError {
    match e {
        // 112 = Windows ERROR_DISK_FULL, 28 = POSIX ENOSPC.
        PlatformError::Io(io) if matches!(io.raw_os_error(), Some(112) | Some(28)) => {
            FlushError::Quota
        }
        _ => FlushError::Other,
    }
}

/// `Date.now()`.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Fixed UTC `YYYY-MM-DD HH:MM:SS` (see [`AutosaveRecord::when`]).
fn format_timestamp(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 → (y, m, d).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
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
}
