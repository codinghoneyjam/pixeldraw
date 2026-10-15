//! Collapsed-panel persistence — `src/ui/shared/collapsible_sections.js`.
//!
//! The JS tree keeps `{...settings, collapsedPanels: {panelId: bool}}` under one
//! localStorage key (`dt.settings.v1`); the native port keeps the same object
//! in `settings.json` so Phase 3.9 (settings persist) shares the file without
//! migration. Reads tolerate a missing or corrupt file (empty map, like JS
//! `loadCollapsed`); writes merge the one id and keep every other field (like
//! JS `persistCollapsed`). Both are best-effort: storage failure must never
//! break the frame loop.

use std::collections::HashMap;

use crate::paths::settings_file;

/// The settings-file field, same name as the JS store field.
pub const COLLAPSED_FIELD: &str = "collapsedPanels";

/// Read the collapsed map; missing/corrupt file or field yields `{}`.
pub fn load_collapsed_panels() -> HashMap<String, bool> {
    let Some(path) = settings_file() else {
        return HashMap::new();
    };
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let root: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    read_collapsed(&root)
}

/// Merge one id into the collapsed map, keeping every other setting.
/// Creates the file (and its directory) when absent.
pub fn save_collapsed_panel(id: &str, collapsed: bool) {
    let Some(path) = settings_file() else {
        return;
    };
    let mut root: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    apply_collapsed(&mut root, id, collapsed);
    if crate::paths::ensure_app_data_dir().is_err() {
        return;
    }
    let text = serde_json::to_string_pretty(&root).unwrap_or_default();
    let _ = std::fs::write(&path, text);
}

/// JS `loadCollapsed`: the field must be a non-array object; entries must be
/// bools, anything else is skipped rather than failing the load.
pub fn read_collapsed(root: &serde_json::Value) -> HashMap<String, bool> {
    root.get(COLLAPSED_FIELD)
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b)))
                .collect()
        })
        .unwrap_or_default()
}

/// JS `persistCollapsed`: merge one id, keep everything else. A non-object
/// root is replaced (a corrupt file must not pin the store).
pub fn apply_collapsed(root: &mut serde_json::Value, id: &str, collapsed: bool) {
    if !root.is_object() {
        *root = serde_json::Value::Object(serde_json::Map::new());
    }
    let obj = root.as_object_mut().expect("object ensured above");
    let entry = obj
        .entry(COLLAPSED_FIELD.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if !entry.is_object() {
        *entry = serde_json::Value::Object(serde_json::Map::new());
    }
    entry
        .as_object_mut()
        .expect("object ensured above")
        .insert(id.to_string(), collapsed.into());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_through_a_value() {
        let mut root = serde_json::Value::Null;
        apply_collapsed(&mut root, "color", true);
        apply_collapsed(&mut root, "brush", false);
        let map = read_collapsed(&root);
        assert_eq!(map.get("color"), Some(&true));
        assert_eq!(map.get("brush"), Some(&false));
    }

    #[test]
    fn other_settings_survive_the_merge() {
        let mut root = json!({"penSize": 4, "collapsedPanels": {"color": true}});
        apply_collapsed(&mut root, "layers", true);
        assert_eq!(root["penSize"], json!(4));
        assert_eq!(root["collapsedPanels"]["color"], json!(true));
        assert_eq!(root["collapsedPanels"]["layers"], json!(true));
    }

    #[test]
    fn corrupt_roots_yield_empty_or_fresh() {
        assert!(read_collapsed(&serde_json::Value::Null).is_empty());
        assert!(read_collapsed(&json!({"collapsedPanels": [1, 2]})).is_empty());
        // Non-bool entries are skipped, not fatal.
        let map = read_collapsed(&json!({"collapsedPanels": {"color": "yes", "brush": true}}));
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("brush"), Some(&true));
        // A corrupt root or field is replaced, never merged into.
        let mut root = json!([1, 2]);
        apply_collapsed(&mut root, "color", true);
        assert!(root.is_object());
        let mut root = json!({"collapsedPanels": false});
        apply_collapsed(&mut root, "color", true);
        assert_eq!(root["collapsedPanels"]["color"], json!(true));
    }

    #[test]
    fn disk_round_trip_preserves_the_file() {
        // Restores on Drop, so a failing assert cannot leak the probe key.
        struct Restore {
            path: std::path::PathBuf,
            backup: Option<String>,
        }
        impl Drop for Restore {
            fn drop(&mut self) {
                match &self.backup {
                    Some(text) => {
                        let _ = std::fs::write(&self.path, text);
                    }
                    None => {
                        let _ = std::fs::remove_file(&self.path);
                    }
                }
            }
        }
        let Some(path) = settings_file() else { return };
        let guard = Restore {
            path: path.clone(),
            backup: std::fs::read_to_string(&path).ok(),
        };
        save_collapsed_panel("test-panel", true);
        let map = load_collapsed_panels();
        assert_eq!(map.get("test-panel"), Some(&true));
        drop(guard);
        // The restore really happened (no residue on green).
        if let Ok(text) = std::fs::read_to_string(&path) {
            let root: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            assert!(!read_collapsed(&root).contains_key("test-panel"));
        }
    }
}
