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

/// Menubar block attachment flags, in File/Edit/View/Layer/Help order.
pub const MENUBAR_DETACHED_FIELD: &str = "menubarDetached";
pub const MENUBAR_BLOCK_COUNT: usize = 5;

/// Settings-file store; `load` once at boot, then `on_event` + `tick`.
pub struct SettingsStore {
    path: Option<PathBuf>,
    /// The persisted recent colors (the panels own the live list).
    recent: Vec<String>,
    /// Detached menubar blocks; the menubar owns the live flags.
    detached: [bool; MENUBAR_BLOCK_COUNT],
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
            detached: [false; MENUBAR_BLOCK_COUNT],
            pending: None,
        }
    }

    pub(crate) fn load_from(path: Option<PathBuf>) -> Self {
        let root = read_root(path.as_deref());
        Self {
            path,
            recent: read_recent(&root),
            detached: read_detached(&root),
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

    /// The detached-block flags loaded at boot.
    pub fn detached(&self) -> [bool; MENUBAR_BLOCK_COUNT] {
        self.detached
    }

    /// Track the menubar's live block layout; a change re-arms the debounce.
    pub fn note_detached(&mut self, detached: &[bool; MENUBAR_BLOCK_COUNT], now: Instant) -> bool {
        if *detached == self.detached {
            return false;
        }
        self.detached = *detached;
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
        obj.insert(
            MENUBAR_DETACHED_FIELD.into(),
            serde_json::Value::Array(
                self.detached
                    .iter()
                    .map(|&detached| serde_json::Value::Bool(detached))
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

/// Unknown/corrupt entries keep the block attached, matching the menubar's
/// default layout.
fn read_detached(root: &serde_json::Value) -> [bool; MENUBAR_BLOCK_COUNT] {
    let mut detached = [false; MENUBAR_BLOCK_COUNT];
    if let Some(values) = root.get(MENUBAR_DETACHED_FIELD).and_then(|v| v.as_array()) {
        for (slot, value) in detached.iter_mut().zip(values) {
            *slot = value.as_bool().unwrap_or(false);
        }
    }
    detached
}

#[cfg(test)]
mod tests;
