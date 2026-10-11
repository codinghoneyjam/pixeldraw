//! The nine setting keys, their value shapes and the frozen defaults.
//!
//! The validators for each live in [`super::validate`]; this half owns the
//! vocabulary the option bar and the tool manager agree on.

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
pub(super) const SHAPE_FILLS: [&str; 2] = ["outline", "fill"];

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
    pub fn parse(s: &str) -> crate::errors::EngineResult<Self> {
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
                return Err(crate::errors::DrawToolError::invalid_state(format!(
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
    pub(super) fn describe(&self) -> String {
        match self {
            Self::Str(s) => (*s).to_string(),
            // Rust's `Display` for f64 prints `200` where JS's `String(200)`
            // does, and `0.5` where it does.
            Self::Num(n) => format!("{n}"),
            Self::Bool(b) => b.to_string(),
        }
    }
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
