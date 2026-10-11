//! The nine UI settings + `validateSetting`.
//!
//! Ported 1:1 from `src/features/document/settings_validator.js`, which
//! `session.js` needs directly. It is **DOM-free** (it imports only
//! `errors`/`events`/`constants`/`pixel`), so it ports intact rather than being
//! deferred to the platform layer — no `localStorage`, no `window`.
//!
//! The interesting part is that every failure here is an `OUT_OF_RANGE`, except
//! an unknown key which is `INVALID_STATE`. That asymmetry is the JS tree's and
//! is preserved: the option bar catches these codes by name.

use crate::errors::{DrawToolError, EngineResult};

/// The tool ids the option bar and the tool manager agree on, frozen.
pub const TOOL_IDS: [&str; 14] = [
    "pen",
    "eraser",
    "eyedropper",
    "fill",
    "line",
    "rect",
    "rrect",
    "ellipse",
    "polygon",
    "arc",
    "chord",
    "radial_gradient",
    "text",
    "hand",
];

/// `SHAPE_FILLS` — the shape tool's two fill modes.
const SHAPE_FILLS: [&str; 2] = ["outline", "fill"];

/// The nine settings keys that carry a change notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    PrimaryColor,
    SecondaryColor,
    PenSize,
    ActiveTool,
    GridMode,
    SnapUnit,
    ShapeFill,
    ShapeRadius,
    ShapeLockAspect,
}

impl SettingKey {
    /// `SETTING_KEYS.includes(key)`, as the first guard of `validateSetting`.
    pub fn parse(s: &str) -> EngineResult<Self> {
        Ok(match s {
            "primaryColor" => Self::PrimaryColor,
            "secondaryColor" => Self::SecondaryColor,
            "penSize" => Self::PenSize,
            "activeTool" => Self::ActiveTool,
            "gridMode" => Self::GridMode,
            "snapUnit" => Self::SnapUnit,
            "shapeFill" => Self::ShapeFill,
            "shapeRadius" => Self::ShapeRadius,
            "shapeLockAspect" => Self::ShapeLockAspect,
            other => {
                return Err(DrawToolError::invalid_state(format!(
                    "unknown setting {other}"
                )));
            }
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrimaryColor => "primaryColor",
            Self::SecondaryColor => "secondaryColor",
            Self::PenSize => "penSize",
            Self::ActiveTool => "activeTool",
            Self::GridMode => "gridMode",
            Self::SnapUnit => "snapUnit",
            Self::ShapeFill => "shapeFill",
            Self::ShapeRadius => "shapeRadius",
            Self::ShapeLockAspect => "shapeLockAspect",
        }
    }
}

/// The canonical value `validateSetting` returns.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    /// `#rrggbb` or `#rrggbbaa`, lower-case.
    Color(String),
    /// `penSize` / `shapeRadius`.
    Int(u32),
    Bool(bool),
    /// `activeTool` / `gridMode` / `shapeFill`.
    Mode(&'static str),
}

/// A setting value as the JS callers passed it — deliberately loose, because
/// `validateSetting` is what rejects the wrong *kind* of value (a number for a
/// colour, a string for a boolean) and that rejection is observable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingInput<'a> {
    Str(&'a str),
    Num(f64),
    Bool(bool),
}

impl SettingInput<'_> {
    /// `String(value)` in the error messages.
    fn describe(&self) -> String {
        match self {
            Self::Str(s) => (*s).to_string(),
            // Rust's `Display` for f64 prints `200` where JS's `String(200)`
            // does, and `0.5` where it does.
            Self::Num(n) => format!("{n}"),
            Self::Bool(b) => b.to_string(),
        }
    }
}

/// `validateColor` — fully transparent paint would silently erase, so alpha 0
/// is rejected. The canonical form keeps alpha only when translucent: opaque
/// stays `#rrggbb`.
fn validate_color(value: SettingInput<'_>) -> EngineResult<SettingValue> {
    let SettingInput::Str(s) = value else {
        return Err(DrawToolError::out_of_range(format!(
            "invalid color {}",
            value.describe()
        )));
    };
    match crate::pixel::parse_hex(s) {
        Some(c) if c[3] != 0 => Ok(SettingValue::Color(if c[3] == 255 {
            crate::pixel::to_hex(c[0] as f64, c[1] as f64, c[2] as f64)
        } else {
            crate::pixel::to_hex8(c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64)
        })),
        _ => Err(DrawToolError::out_of_range(format!(
            "invalid color {}",
            value.describe()
        ))),
    }
}

/// `validateInt(value, min, max, what)` — JS `Number.isInteger` semantics.
fn validate_int(value: SettingInput<'_>, min: u32, max: u32, what: &str) -> EngineResult<u32> {
    let SettingInput::Num(n) = value else {
        return Err(DrawToolError::out_of_range(format!(
            "invalid {what} {}",
            value.describe()
        )));
    };
    if !n.is_finite() || n.fract() != 0.0 || n < min as f64 || n > max as f64 {
        return Err(DrawToolError::out_of_range(format!(
            "invalid {what} {}",
            value.describe()
        )));
    }
    Ok(n as u32)
}

fn validate_bool(value: SettingInput<'_>, key: SettingKey) -> EngineResult<SettingValue> {
    match value {
        SettingInput::Bool(b) => Ok(SettingValue::Bool(b)),
        other => Err(DrawToolError::out_of_range(format!(
            "invalid {} {}",
            key.as_str(),
            other.describe()
        ))),
    }
}

fn validate_member(
    value: SettingInput<'_>,
    allowed: &[&'static str],
    what: &str,
) -> EngineResult<SettingValue> {
    let SettingInput::Str(s) = value else {
        return Err(DrawToolError::out_of_range(format!(
            "unknown {what} {}",
            value.describe()
        )));
    };
    match allowed.iter().find(|m| **m == s) {
        Some(m) => Ok(SettingValue::Mode(m)),
        None => Err(DrawToolError::out_of_range(format!(
            "unknown {what} {}",
            value.describe()
        ))),
    }
}

// <META - ROLE : validateSetting from settings_validator.js L42-66 | L42-66>
/// Validate one setting, returning its key and canonical value.
///
/// The final `throw` in the JS is unreachable for its own constants; the
/// exhaustive `match` here makes that a compile-time fact instead.
pub fn validate_setting(
    key: &str,
    value: SettingInput<'_>,
) -> EngineResult<(SettingKey, SettingValue)> {
    let key = SettingKey::parse(key)?;
    let value = match key {
        SettingKey::PrimaryColor | SettingKey::SecondaryColor => validate_color(value)?,
        SettingKey::PenSize => SettingValue::Int(validate_int(value, 1, 64, "penSize")?),
        SettingKey::ActiveTool => validate_member(value, &TOOL_IDS, "tool")?,
        SettingKey::GridMode => validate_member(value, &crate::constants::GRID_MODES, "gridMode")?,
        SettingKey::SnapUnit | SettingKey::ShapeLockAspect => validate_bool(value, key)?,
        SettingKey::ShapeFill => validate_member(value, &SHAPE_FILLS, "shapeFill")?,
        SettingKey::ShapeRadius => SettingValue::Int(validate_int(value, 0, 2048, "shapeRadius")?),
    };
    Ok((key, value))
}

/// `DEFAULT_SETTINGS` — the frozen starting point of every `Session`.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub primary_color: String,
    pub secondary_color: String,
    pub pen_size: u32,
    pub active_tool: &'static str,
    pub grid_mode: &'static str,
    pub snap_unit: bool,
    pub shape_fill: &'static str,
    pub shape_radius: u32,
    pub shape_lock_aspect: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            primary_color: "#000000".to_string(),
            secondary_color: "#ffffff".to_string(),
            pen_size: 1,
            active_tool: "pen",
            grid_mode: "off",
            snap_unit: false,
            shape_fill: "outline",
            shape_radius: 8,
            shape_lock_aspect: false,
        }
    }
}

impl Settings {
    pub fn get(&self, key: SettingKey) -> SettingValue {
        match key {
            SettingKey::PrimaryColor => SettingValue::Color(self.primary_color.clone()),
            SettingKey::SecondaryColor => SettingValue::Color(self.secondary_color.clone()),
            SettingKey::PenSize => SettingValue::Int(self.pen_size),
            SettingKey::ActiveTool => SettingValue::Mode(self.active_tool),
            SettingKey::GridMode => SettingValue::Mode(self.grid_mode),
            SettingKey::SnapUnit => SettingValue::Bool(self.snap_unit),
            SettingKey::ShapeFill => SettingValue::Mode(self.shape_fill),
            SettingKey::ShapeRadius => SettingValue::Int(self.shape_radius),
            SettingKey::ShapeLockAspect => SettingValue::Bool(self.shape_lock_aspect),
        }
    }

    /// Store an already-validated value.
    ///
    /// Returns `false` when the value is what was already stored — the JS
    /// short-circuit in `setSetting` that skips the change event.
    pub fn set(&mut self, key: SettingKey, value: SettingValue) -> bool {
        if self.get(key) == value {
            return false;
        }
        match (key, value) {
            (SettingKey::PrimaryColor, SettingValue::Color(c)) => self.primary_color = c,
            (SettingKey::SecondaryColor, SettingValue::Color(c)) => self.secondary_color = c,
            (SettingKey::PenSize, SettingValue::Int(n)) => self.pen_size = n,
            (SettingKey::ActiveTool, SettingValue::Mode(m)) => self.active_tool = m,
            (SettingKey::GridMode, SettingValue::Mode(m)) => self.grid_mode = m,
            (SettingKey::SnapUnit, SettingValue::Bool(b)) => self.snap_unit = b,
            (SettingKey::ShapeFill, SettingValue::Mode(m)) => self.shape_fill = m,
            (SettingKey::ShapeRadius, SettingValue::Int(n)) => self.shape_radius = n,
            (SettingKey::ShapeLockAspect, SettingValue::Bool(b)) => self.shape_lock_aspect = b,
            // A tag the key does not accept cannot get here: `validate_setting`
            // is the only producer, and it tags by key.
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every default must itself pass `validate_setting`, which needs the value
    /// back as a loose input.
    fn default_passes_validation() {
        let s = Settings::default();
        assert!(validate_setting("primaryColor", SettingInput::Str(&s.primary_color)).is_ok());
        assert!(validate_setting("secondaryColor", SettingInput::Str(&s.secondary_color)).is_ok());
        assert!(validate_setting("penSize", SettingInput::Num(f64::from(s.pen_size))).is_ok());
        assert!(validate_setting("activeTool", SettingInput::Str(s.active_tool)).is_ok());
        assert!(validate_setting("gridMode", SettingInput::Str(s.grid_mode)).is_ok());
        assert!(validate_setting("snapUnit", SettingInput::Bool(s.snap_unit)).is_ok());
        assert!(validate_setting("shapeFill", SettingInput::Str(s.shape_fill)).is_ok());
        assert!(
            validate_setting("shapeRadius", SettingInput::Num(f64::from(s.shape_radius))).is_ok()
        );
        assert!(
            validate_setting("shapeLockAspect", SettingInput::Bool(s.shape_lock_aspect)).is_ok()
        );
    }

    // <META - ROLE : parity with tests/model_basic.test.mjs B5.5 settings cases | L196-211>
    #[test]
    fn setting_failures_carry_the_js_codes() {
        assert_eq!(
            validate_setting("penSize", SettingInput::Num(200.0))
                .unwrap_err()
                .code,
            crate::errors::CODE_OUT_OF_RANGE
        );
        assert_eq!(
            validate_setting("primaryColor", SettingInput::Str("red"))
                .unwrap_err()
                .code,
            crate::errors::CODE_OUT_OF_RANGE
        );
        assert_eq!(
            validate_setting("activeTool", SettingInput::Str("airbrush"))
                .unwrap_err()
                .code,
            crate::errors::CODE_OUT_OF_RANGE
        );
        assert_eq!(
            validate_setting("nope", SettingInput::Num(1.0))
                .unwrap_err()
                .code,
            crate::errors::CODE_INVALID_STATE
        );
        // The rejected colour is fully transparent — it would erase silently.
        assert!(validate_setting("primaryColor", SettingInput::Str("#ff000000")).is_err());
        assert!(validate_setting("snapUnit", SettingInput::Num(1.0)).is_err());
        assert!(validate_setting("shapeFill", SettingInput::Str("both")).is_err());
        assert!(validate_setting("penSize", SettingInput::Str("8")).is_err());
        assert!(validate_setting("shapeRadius", SettingInput::Num(-1.0)).is_err());
        assert!(validate_setting("shapeRadius", SettingInput::Num(2049.0)).is_err());
        assert!(validate_setting("shapeRadius", SettingInput::Num(2048.0)).is_ok());
        assert!(validate_setting("gridMode", SettingInput::Str("unit")).is_ok());
        assert!(validate_setting("gridMode", SettingInput::Str("hex")).is_err());
        for t in TOOL_IDS {
            assert!(
                validate_setting("activeTool", SettingInput::Str(t)).is_ok(),
                "{t}"
            );
        }
    }

    #[test]
    fn colours_canonicalise_like_the_js() {
        let opaque = validate_setting("primaryColor", SettingInput::Str("#FF0000")).expect("ok");
        assert_eq!(opaque.1, SettingValue::Color("#ff0000".into()));
        let short = validate_setting("primaryColor", SettingInput::Str("#f00")).expect("ok");
        assert_eq!(short.1, SettingValue::Color("#ff0000".into()));
        // Translucent keeps its alpha.
        let alpha = validate_setting("primaryColor", SettingInput::Str("#ff000080")).expect("ok");
        assert_eq!(alpha.1, SettingValue::Color("#ff000080".into()));
    }

    #[test]
    fn defaults_are_the_frozen_table() {
        let s = Settings::default();
        assert_eq!(s.primary_color, "#000000");
        assert_eq!(s.secondary_color, "#ffffff");
        assert_eq!(s.pen_size, 1);
        assert_eq!(s.active_tool, "pen");
        assert_eq!(s.grid_mode, "off");
        assert!(!s.snap_unit);
        assert_eq!(s.shape_fill, "outline");
        assert_eq!(s.shape_radius, 8);
        assert!(!s.shape_lock_aspect);
        default_passes_validation();
    }

    #[test]
    fn set_reports_whether_the_value_changed() {
        let mut s = Settings::default();
        assert!(!s.set(SettingKey::PenSize, SettingValue::Int(1)));
        assert!(s.set(SettingKey::PenSize, SettingValue::Int(8)));
        assert_eq!(s.pen_size, 8);
        // A mismatched tag is not a write.
        assert!(!s.set(SettingKey::PenSize, SettingValue::Bool(true)));
        assert_eq!(s.pen_size, 8);
    }

    #[test]
    fn keys_round_trip_through_the_js_strings() {
        for k in [
            SettingKey::PrimaryColor,
            SettingKey::SecondaryColor,
            SettingKey::PenSize,
            SettingKey::ActiveTool,
            SettingKey::GridMode,
            SettingKey::SnapUnit,
            SettingKey::ShapeFill,
            SettingKey::ShapeRadius,
            SettingKey::ShapeLockAspect,
        ] {
            assert_eq!(SettingKey::parse(k.as_str()).expect("known"), k);
            assert!(crate::events::SETTING_KEYS.contains(&k.as_str()));
        }
        assert_eq!(
            SettingKey::parse("nope").unwrap_err().code,
            crate::errors::CODE_INVALID_STATE
        );
    }
}
