//! The nine UI settings + `validateSetting` — facade.
//!
//! Ported 1:1 from `src/features/document/settings_validator.js`, which
//! `session.js` needs directly. It is **DOM-free** (it imports only
//! `errors`/`events`/`constants`/`pixel`), so it ports intact rather than being
//! deferred to the platform layer — no `localStorage`, no `window`.
//!
//! The interesting part is that every failure here is an `OUT_OF_RANGE`, except
//! an unknown key which is `INVALID_STATE`. That asymmetry is the JS tree's and
//! is preserved: the option bar catches these codes by name.

mod types;
mod validate;

pub use types::{SettingInput, SettingKey, SettingValue, Settings, TOOL_IDS};
pub use validate::validate_setting;
