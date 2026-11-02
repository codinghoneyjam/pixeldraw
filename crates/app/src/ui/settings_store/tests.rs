//! Settings persistence regression tests.

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
    assert_eq!(
        root[MENUBAR_DETACHED_FIELD],
        json!([false, false, false, false, false])
    );
}

#[test]
fn detached_blocks_load_normalise_and_persist() {
    let file = TempFile::new("menubar-detached");
    file.write(r#"{"menubarDetached": [true, "nope", false, true]}"#);
    let mut store = SettingsStore::load_from(Some(file.0.clone()));
    assert_eq!(store.detached(), [true, false, false, true, false]);
    let t0 = Instant::now();
    assert!(store.note_detached(&[false, true, false, true, false], t0));
    assert!(!store.note_detached(&[false, true, false, true, false], t0));
    let session = Session::new();
    assert!(store.tick(t0 + DEBOUNCE, &session));
    assert_eq!(
        file.read_json()[MENUBAR_DETACHED_FIELD],
        json!([false, true, false, true, false])
    );
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
