//! `validateSetting` — per-key validation, producing the canonical value.
//!
//! Every failure here is an `OUT_OF_RANGE` except an unknown key, which is an
//! `INVALID_STATE`; tests/model_basic.test.mjs B5.5 pins the split, and the
//! option bar catches these codes by name.

use super::types::{SHAPE_FILLS, SettingInput as Input, SettingKey, SettingValue, TOOL_IDS};
use crate::constants::GRID_MODES;
use crate::errors::{DrawToolError, EngineResult};

/// `validateColor` — fully transparent paint would silently erase, so alpha 0
/// is rejected. The canonical form keeps alpha only when translucent: opaque
/// stays `#rrggbb`.
fn validate_color(value: Input) -> EngineResult<SettingValue> {
    let Input::Str(s) = value else {
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
fn validate_int(value: Input, min: u32, max: u32, what: &str) -> EngineResult<u32> {
    let Input::Num(n) = value else {
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

fn validate_bool(value: Input, key: SettingKey) -> EngineResult<SettingValue> {
    match value {
        Input::Bool(b) => Ok(SettingValue::Bool(b)),
        other => Err(DrawToolError::out_of_range(format!(
            "invalid {} {}",
            key.as_str(),
            other.describe()
        ))),
    }
}

fn validate_member(
    value: Input,
    allowed: &[&'static str],
    what: &str,
) -> EngineResult<SettingValue> {
    let Input::Str(s) = value else {
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
pub fn validate_setting(key: &str, value: Input) -> EngineResult<(SettingKey, SettingValue)> {
    let key = SettingKey::parse(key)?;
    let value = match key {
        SettingKey::PrimaryColor | SettingKey::SecondaryColor => validate_color(value)?,
        SettingKey::PenSize => SettingValue::Int(validate_int(value, 1, 64, "penSize")?),
        SettingKey::ActiveTool => validate_member(value, &TOOL_IDS, "tool")?,
        SettingKey::GridMode => validate_member(value, &GRID_MODES, "gridMode")?,
        SettingKey::SnapUnit | SettingKey::ShapeLockAspect => validate_bool(value, key)?,
        SettingKey::ShapeFill => validate_member(value, &SHAPE_FILLS, "shapeFill")?,
        SettingKey::ShapeRadius => SettingValue::Int(validate_int(value, 0, 2048, "shapeRadius")?),
    };
    Ok((key, value))
}

#[cfg(test)]
mod tests {
    use super::super::types::{SettingInput, Settings};
    use super::*;
    use crate::errors::CODE_INVALID_STATE;
    use crate::events::SETTING_KEYS;

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
            CODE_INVALID_STATE
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
            assert!(SETTING_KEYS.contains(&k.as_str()));
        }
        assert_eq!(
            SettingKey::parse("nope").unwrap_err().code,
            CODE_INVALID_STATE
        );
    }
}
