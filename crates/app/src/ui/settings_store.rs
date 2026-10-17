//! Settings persistence — `src/ui/shared/storage.js` (`restoreSettings`,
//! `attachSettingsPersist`, `loadRecentColors`) over the native
//! `settings.json` that `pixel_platform::ui_prefs` already owns (Phase 3.9).
//!
//! One JSON object at `%APPDATA%/pixeldraw/settings.json`; field names keep the
//! JS shapes: the nine `dt.settings.v1` keys at the root, `recentColors`
//! (`dt.recentColors`) as a string array, `collapsedPanels` (written by
//! `platform::save_collapsed_panel`). **D9 fix**: JS `attachSettingsPersist`
//! rewrote `dt.settings.v1` with only the nine keys and wiped
//! `collapsedPanels`; every save here re-reads the file and merges, so nothing
//! outside the nine keys is ever dropped.
//!
//! Timing is injected (`Instant`): the frame loop calls [`SettingsStore::tick`]
//! and owns the 250 ms trailing debounce, mirroring the JS
//! `clearTimeout` + `setTimeout` pair.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use pixel_engine::model::{EventDetail, ModelEvent, Session, SettingInput};

/// JS `PERSIST_KEYS` order; `activeTool` last (restore order is observable).
pub const PERSIST_KEYS: [&str; 9] = [
    "primaryColor",
    "secondaryColor",
    "penSize",
    "gridMode",
    "snapUnit",
    "shapeFill",
    "shapeRadius",
    "shapeLockAspect",
    "activeTool",
];

/// `RECENT_MAX` — `color_constants.js` `RECENT_COLOR_LIMIT`.
pub const RECENT_MAX: usize = 8;

/// The JS debounce in `attachSettingsPersist` (250 ms, trailing).
pub const DEBOUNCE: Duration = Duration::from_millis(250);

/// The recent-colors field inside the settings file (`dt.recentColors`).
pub const RECENT_FIELD: &str = "recentColors";

/// Settings-file store; `load` once at boot, then `on_event` + `tick`.
pub struct SettingsStore {
    path: Option<PathBuf>,
    /// The persisted recent colors (the panels own the live list).
    recent: Vec<String>,
    /// Debounce start of the last change; `None` = nothing to write.
    pending: Option<Instant>,
}

impl SettingsStore {
    /// Boot: read the file once. Corrupt files read as empty everywhere
    /// (JS `loadJson` swallows every error).
    pub fn load() -> Self {
        Self::load_from(pixel_platform::settings_file())
    }

    /// No file: nothing read or written (headless tests, non-shell callers).
    pub(crate) fn empty() -> Self {
        Self {
            path: None,
            recent: Vec::new(),
            pending: None,
        }
    }

    pub(crate) fn load_from(path: Option<PathBuf>) -> Self {
        let root = read_root(path.as_deref());
        Self {
            path,
            recent: read_recent(&root),
            pending: None,
        }
    }

    /// JS `restoreSettings`: apply the nine keys in `PERSIST_KEYS` order,
    /// skipping keys the file lacks and values the validator rejects
    /// (`set_setting` returning `Err` is the per-key try/catch).
    pub fn restore_into(&self, session: &mut Session) {
        let root = read_root(self.path.as_deref());
        for key in PERSIST_KEYS {
            let Some(value) = root.get(key) else { continue };
            let input = match value {
                serde_json::Value::String(s) => SettingInput::Str(s),
                serde_json::Value::Number(n) => match n.as_f64() {
                    Some(f) => SettingInput::Num(f),
                    None => continue,
                },
                serde_json::Value::Bool(b) => SettingInput::Bool(*b),
                _ => continue,
            };
            let _ = session.set_setting(key, input);
        }
    }

    /// The loaded recent colors (`loadRecentColors` filtering already applied).
    pub fn recent(&self) -> &[String] {
        &self.recent
    }

    /// Track the panel's live list; a change re-arms the debounce. Returns
    /// whether the list moved.
    pub fn note_recent(&mut self, list: &[String], now: Instant) -> bool {
        let clipped: Vec<String> = list.iter().take(RECENT_MAX).cloned().collect();
        if clipped == self.recent {
            return false;
        }
        self.recent = clipped;
        self.pending = Some(now);
        true
    }

    /// Any `settings-changed` event arms the debounce (JS listener).
    pub fn on_event(&mut self, ev: &ModelEvent, now: Instant) {
        if matches!(ev.detail, EventDetail::Settings { .. }) {
            self.pending = Some(now);
        }
    }

    /// Trailing debounce: write 250 ms after the *last* change. Returns
    /// whether a write happened this call (tests); write failures are
    /// best-effort, like JS `storeJson`.
    pub fn tick(&mut self, now: Instant, session: &Session) -> bool {
        let Some(start) = self.pending else {
            return false;
        };
        if now.saturating_duration_since(start) < DEBOUNCE {
            return false;
        }
        self.pending = None;
        self.save(session);
        true
    }

    /// Merge-save: re-read the file, overlay the nine keys and the recent
    /// colors, keep every other field (`collapsedPanels` and unknown keys).
    fn save(&self, session: &Session) {
        let Some(path) = &self.path else { return };
        if pixel_platform::ensure_app_data_dir().is_err() {
            return;
        }
        let mut root = read_root(Some(path));
        let Some(obj) = root.as_object_mut() else {
            return;
        };
        let s = session.settings();
        obj.insert("primaryColor".into(), s.primary_color.into());
        obj.insert("secondaryColor".into(), s.secondary_color.into());
        obj.insert("penSize".into(), s.pen_size.into());
        obj.insert("gridMode".into(), s.grid_mode.into());
        obj.insert("snapUnit".into(), s.snap_unit.into());
        obj.insert("shapeFill".into(), s.shape_fill.into());
        obj.insert("shapeRadius".into(), s.shape_radius.into());
        obj.insert("shapeLockAspect".into(), s.shape_lock_aspect.into());
        obj.insert("activeTool".into(), s.active_tool.into());
        obj.insert(
            RECENT_FIELD.into(),
            serde_json::Value::Array(
                self.recent
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
        let text = serde_json::to_string_pretty(&root).unwrap_or_default();
        let _ = pixel_platform::write_bytes_atomic(path, text.as_bytes());
    }
}

/// Read the settings file as a JSON object; missing/corrupt reads as `{}`.
fn read_root(path: Option<&std::path::Path>) -> serde_json::Value {
    let text = path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    if value.is_object() {
        value
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    }
}

/// JS `loadRecentColors`: array in, strings only, first `RECENT_MAX`.
fn read_recent(root: &serde_json::Value) -> Vec<String> {
    root.get(RECENT_FIELD)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .take(RECENT_MAX)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixel_engine::model::{SettingKey, SettingValue};
    use serde_json::json;

    /// One temp settings file, removed on drop.
    struct TempFile(PathBuf);
    impl TempFile {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "pixeldraw-settings-{tag}-{}.json",
                std::process::id()
            ));
            let _ = std::fs::remove_file(&path);
            Self(path)
        }
        fn write(&self, text: &str) {
            std::fs::write(&self.0, text).expect("probe write");
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

    fn settings_event() -> ModelEvent {
        ModelEvent::settings_changed(SettingKey::PenSize, SettingValue::Int(4))
    }

    #[test]
    fn restore_applies_valid_keys_and_skips_corrupt_ones() {
        let file = TempFile::new("restore");
        file.write(
            r##"{
              "primaryColor": "#123456",
              "penSize": "nope",
              "gridMode": "tile",
              "snapUnit": true,
              "shapeRadius": 99,
              "activeTool": "eraser",
              "collapsedPanels": {"color": true}
            }"##,
        );
        let store = SettingsStore::load_from(Some(file.0.clone()));
        let mut session = Session::new();
        store.restore_into(&mut session);
        let s = session.settings();
        assert_eq!(s.primary_color, "#123456");
        assert_eq!(s.pen_size, 1, "corrupt penSize is skipped, not fatal");
        assert_eq!(s.grid_mode, "tile");
        assert!(s.snap_unit);
        assert_eq!(s.shape_radius, 99);
        assert_eq!(s.active_tool, "eraser", "activeTool restores last");
    }

    #[test]
    fn restore_of_a_missing_file_is_a_noop() {
        let file = TempFile::new("missing");
        let store = SettingsStore::load_from(Some(file.0.clone()));
        let mut session = Session::new();
        store.restore_into(&mut session);
        assert_eq!(session.settings(), Session::new().settings());
    }

    #[test]
    fn corrupt_json_loads_as_defaults() {
        let file = TempFile::new("corrupt");
        file.write("{not json");
        let store = SettingsStore::load_from(Some(file.0.clone()));
        assert!(store.recent().is_empty());
        let mut session = Session::new();
        store.restore_into(&mut session);
        assert_eq!(session.settings(), Session::new().settings());
    }

    #[test]
    fn recent_colors_load_filters_and_caps() {
        let file = TempFile::new("recent");
        file.write(
            r#"{"recentColors": [1, "red", "blue", "green", "a", "b", "c", "d", "e", "f", "g"]}"#,
        );
        let store = SettingsStore::load_from(Some(file.0.clone()));
        let recent = store.recent().to_vec();
        assert_eq!(recent.len(), RECENT_MAX);
        assert_eq!(recent[0], "red");
        assert_eq!(recent[7], "e", "first 8 strings, in order");
    }

    #[test]
    fn debounce_is_trailing_and_resets_on_new_events() {
        let file = TempFile::new("debounce");
        let mut store = SettingsStore::load_from(Some(file.0.clone()));
        let mut session = Session::new();
        session
            .set_setting("penSize", SettingInput::Num(4.0))
            .expect("valid");
        let t0 = Instant::now();
        store.on_event(&settings_event(), t0);
        store.on_event(&settings_event(), t0 + Duration::from_millis(100));
        // Last change at +100 ms → due at +350 ms, not +250 ms.
        assert!(!store.tick(t0 + Duration::from_millis(300), &session));
        assert!(store.tick(t0 + Duration::from_millis(400), &session));
        assert_eq!(file.read_json()["penSize"], json!(4));
    }

    #[test]
    fn save_merges_and_keeps_collapsed_panels_and_unknown_keys() {
        let file = TempFile::new("merge");
        file.write(r#"{"collapsedPanels": {"color": true}, "unknown": 7}"#);
        let mut store = SettingsStore::load_from(Some(file.0.clone()));
        let mut session = Session::new();
        session
            .set_setting("penSize", SettingInput::Num(4.0))
            .expect("valid");
        let t0 = Instant::now();
        store.on_event(&settings_event(), t0);
        assert!(store.tick(t0 + DEBOUNCE, &session));
        let root = file.read_json();
        assert_eq!(root["collapsedPanels"]["color"], json!(true), "D9 fix");
        assert_eq!(root["unknown"], json!(7));
        assert_eq!(root["penSize"], json!(4));
        assert_eq!(root["activeTool"], json!("pen"));
        assert_eq!(root["recentColors"], json!([]));
    }

    #[test]
    fn recent_change_alone_triggers_a_save() {
        let file = TempFile::new("recent-save");
        file.write(r#"{"penSize": 1}"#);
        let mut store = SettingsStore::load_from(Some(file.0.clone()));
        let session = Session::new();
        let t0 = Instant::now();
        assert!(store.note_recent(&["#aabbcc".to_string()], t0));
        assert!(!store.tick(t0 + Duration::from_millis(100), &session));
        assert!(store.tick(t0 + DEBOUNCE, &session));
        assert_eq!(file.read_json()["recentColors"], json!(["#aabbcc"]));
        // Repeating the same list is not a change.
        assert!(!store.note_recent(&["#aabbcc".to_string()], t0));
    }
}
