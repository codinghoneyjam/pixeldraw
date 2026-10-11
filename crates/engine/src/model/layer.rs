//! Layer — the canvas-sized bitmap plus its display properties.
//!
//! Ported 1:1 from `src/features/layers/layer.js`. A `Layer` is an owning value:
//! the JS class held a reference to a `ChunkStore` and commands moved layers
//! around by identity, which Rust's `Vec<Layer>` reproduces by value.

use crate::blend::is_blend_mode;
use crate::chunkstore::ChunkStore;
use crate::composite::LayerSource;
use crate::errors::{DrawToolError, EngineResult};

/// `BLEND_MODES` is re-exported here so `model` consumers keep one import site,
/// exactly as the JS file did.
pub use crate::blend::{BLEND_MODES as LAYER_BLEND_MODES, is_blend_mode as is_layer_blend};

/// The five mutable layer properties, in the JS `Object.freeze` order.
pub const LAYER_PROP_FIELDS: [&str; 5] = ["name", "visible", "locked", "opacity", "blend"];

// <META - ROLE : the property validators from layer.js L11-29 | L11-29>
/// `typeof name !== "string" || length 0 || >128 || blank` → `INVALID_STATE`.
///
/// `length` is the JS `String.prototype.length`, i.e. **UTF-16 code units**, not
/// code points: an emoji name is 2 long in JS and 1 in `chars().count()`. The
/// encoder is spelled out so a 4-emoji name is rejected at 128 either way.
pub fn validate_layer_name(name: &str) -> EngineResult<()> {
    if name.encode_utf16().count() == 0
        || name.encode_utf16().count() > 128
        || name.trim().is_empty()
    {
        return Err(DrawToolError::invalid_state(
            "layer name must be 1-128 non-blank chars",
        ));
    }
    Ok(())
}

/// `typeof v !== "number" || !Number.isFinite(v) || v<0 || v>1` → `OUT_OF_RANGE`.
pub fn validate_layer_opacity(opacity: f64) -> EngineResult<()> {
    if !opacity.is_finite() || opacity < 0.0 || opacity > 1.0 {
        return Err(DrawToolError::out_of_range(
            "layer opacity must be a finite number in [0,1]",
        ));
    }
    Ok(())
}

/// Membership, not just a non-empty string: a `"dissolve"` blend must be
/// rejected rather than silently demoted.
pub fn validate_layer_blend(blend: &str) -> EngineResult<()> {
    if !is_blend_mode(blend) {
        return Err(DrawToolError::invalid_state(format!(
            "unknown layer blend {blend}"
        )));
    }
    Ok(())
}

/// The five property fields, as an enum.
///
/// The JS tree passed the field as a bare string and `setProp` raised
/// `INVALID_STATE` for anything outside [`LAYER_PROP_FIELDS`]; the enum makes
/// that unreachable at the type level, and [`LayerField::parse`] keeps the
/// error for values that arrive as text (schema/vector import in Phase 1.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerField {
    Name,
    Visible,
    Locked,
    Opacity,
    Blend,
}

impl LayerField {
    pub fn parse(s: &str) -> EngineResult<Self> {
        Ok(match s {
            "name" => Self::Name,
            "visible" => Self::Visible,
            "locked" => Self::Locked,
            "opacity" => Self::Opacity,
            "blend" => Self::Blend,
            other => {
                return Err(DrawToolError::invalid_state(format!(
                    "unknown layer field {other}"
                )));
            }
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Visible => "visible",
            Self::Locked => "locked",
            Self::Opacity => "opacity",
            Self::Blend => "blend",
        }
    }
}

/// A property value, tagged by which field it belongs to.
///
/// The JS `setProp(field, value)` accepted anything and coerced: `visible` and
/// `locked` were `value === true`, so `setProp("visible", 1)` silently stored
/// `false`. Tagging the value makes that coercion impossible, which is the one
/// deliberate divergence in this file — every producer in
/// [`crate::model::operations`] already passed the matching type, so no JS
/// behaviour is lost by it.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerPropValue {
    Name(String),
    Visible(bool),
    Locked(bool),
    Opacity(f64),
    Blend(&'static str),
}

impl LayerPropValue {
    fn field(&self) -> LayerField {
        match self {
            Self::Name(_) => LayerField::Name,
            Self::Visible(_) => LayerField::Visible,
            Self::Locked(_) => LayerField::Locked,
            Self::Opacity(_) => LayerField::Opacity,
            Self::Blend(_) => LayerField::Blend,
        }
    }
}

/// One layer: a store sized to the canvas, plus the five display properties.
#[derive(Debug, Clone)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f64,
    pub blend: &'static str,
    pub store: ChunkStore,
}

impl Layer {
    /// The JS constructor. Validates all three constrained properties.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        visible: bool,
        locked: bool,
        opacity: f64,
        blend: &'static str,
        store: ChunkStore,
    ) -> EngineResult<Self> {
        let name = name.into();
        validate_layer_name(&name)?;
        validate_layer_opacity(opacity)?;
        validate_layer_blend(blend)?;
        Ok(Self {
            id: id.into(),
            name,
            visible,
            locked,
            opacity,
            blend,
            store,
        })
    }

    /// `Layer.create` — the defaults a freshly added layer gets.
    pub fn create(
        id: impl Into<String>,
        name: impl Into<String>,
        width_px: u32,
        height_px: u32,
    ) -> EngineResult<Self> {
        crate::constants::require_valid_canvas_size(width_px, height_px)?;
        let name = name.into();
        validate_layer_name(&name)?;
        Self::new(
            id,
            name,
            true,
            false,
            1.0,
            "normal",
            ChunkStore::new(width_px, height_px)?,
        )
    }

    /// `cloneWith({id, name})` — the duplicate path. The store is deep-cloned.
    pub fn clone_with(&self, id: impl Into<String>, name: impl Into<String>) -> EngineResult<Self> {
        let name = name.into();
        validate_layer_name(&name)?;
        Ok(Self {
            id: id.into(),
            name,
            visible: self.visible,
            locked: self.locked,
            opacity: self.opacity,
            blend: self.blend,
            store: self.store.clone_deep(),
        })
    }

    /// `setProp(field, value)`.
    pub fn set_prop(&mut self, field: LayerField, value: LayerPropValue) -> EngineResult<()> {
        // The JS switch silently coerced `visible`/`locked` with `value === true`;
        // the tagged value makes a field/value mismatch an explicit failure.
        if field != value.field() {
            return Err(DrawToolError::invalid_state(format!(
                "layer field {} does not match its value",
                field.as_str()
            )));
        }
        match (field, value) {
            (LayerField::Name, LayerPropValue::Name(n)) => {
                validate_layer_name(&n)?;
                self.name = n;
            }
            (LayerField::Visible, LayerPropValue::Visible(b)) => self.visible = b,
            (LayerField::Locked, LayerPropValue::Locked(b)) => self.locked = b,
            (LayerField::Opacity, LayerPropValue::Opacity(o)) => {
                validate_layer_opacity(o)?;
                self.opacity = o;
            }
            (LayerField::Blend, LayerPropValue::Blend(b)) => {
                validate_layer_blend(b)?;
                self.blend = b;
            }
            _ => unreachable!("field and value tags were checked above"),
        }
        Ok(())
    }

    /// Feed the compositor without letting it depend on this module.
    ///
    /// [`crate::composite`] is a lower layer than `model`, so it defines the
    /// plain [`LayerSource`] struct and the document side builds one here — the
    /// dependency arrow stays `composite ← model` and never becomes a cycle.
    pub fn as_source(&self) -> LayerSource<'_> {
        LayerSource {
            store: Some(&self.store),
            opacity: self.opacity,
            mode: self.blend,
            visible: self.visible,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend::BLEND_MODES;
    use crate::pixel::pack_rgba;

    fn blank_layer(id: &str) -> Layer {
        Layer::create(id, "Layer 1", 32, 32).expect("valid size")
    }

    // <META - ROLE : parity with tests/model_basic.test.mjs B5.1 name guards | L42-43>
    #[test]
    fn layer_name_guards() {
        assert!(validate_layer_name("ok").is_ok());
        assert!(validate_layer_name("").is_err());
        assert!(validate_layer_name("   ").is_err());
        let long = "x".repeat(129);
        assert!(validate_layer_name(&long).is_err());
        let max = "x".repeat(128);
        assert!(validate_layer_name(&max).is_ok());
        // UTF-16 units, not code points — a 65-emoji name is 130 units in JS.
        let emoji = "😀".repeat(65);
        assert!(validate_layer_name(&emoji).is_err());
        let few = "😀".repeat(64);
        assert!(validate_layer_name(&few).is_ok());
    }

    #[test]
    fn opacity_guard_matches_the_js_number_predicate() {
        assert!(validate_layer_opacity(0.0).is_ok());
        assert!(validate_layer_opacity(1.0).is_ok());
        assert!(validate_layer_opacity(0.5).is_ok());
        assert!(validate_layer_opacity(-0.1).is_err());
        assert!(validate_layer_opacity(1.1).is_err());
        assert!(validate_layer_opacity(f64::NAN).is_err());
        assert!(validate_layer_opacity(f64::INFINITY).is_err());
    }

    #[test]
    fn blend_guard_is_membership_not_non_empty() {
        for m in BLEND_MODES {
            assert!(validate_layer_blend(m).is_ok(), "{m}");
        }
        // "dissolve" must be rejected rather than demoted to normal.
        assert!(validate_layer_blend("dissolve").is_err());
        assert!(validate_layer_blend("").is_err());
    }

    #[test]
    fn create_defaults_and_bad_canvas() {
        let l = Layer::create("layer-0001", "Layer 1", 64, 64).expect("valid");
        assert_eq!(l.id, "layer-0001");
        assert!(l.visible);
        assert!(!l.locked);
        assert_eq!(l.opacity, 1.0);
        assert_eq!(l.blend, "normal");
        assert_eq!(l.store.width_px, 64);
        assert!(Layer::create("a", "x", 31, 32).is_err());
        assert!(Layer::create("a", "  ", 32, 32).is_err());
    }

    #[test]
    fn clone_with_deep_copies_the_store() {
        let mut l = blank_layer("layer-0001");
        l.store.set_pixel(3, 3, pack_rgba(255.0, 0.0, 0.0, 255.0));
        let mut c = l.clone_with("layer-0002", "Layer 1 copy").expect("valid");
        assert_eq!(c.id, "layer-0002");
        assert_eq!(c.name, "Layer 1 copy");
        assert_eq!(c.store.get_pixel(3, 3), l.store.get_pixel(3, 3));
        c.store.set_pixel(3, 3, pack_rgba(0.0, 255.0, 0.0, 255.0));
        assert_eq!(l.store.get_pixel(3, 3), 0xff0000ff);
    }

    #[test]
    fn set_prop_writes_and_validates() {
        let mut l = blank_layer("layer-0001");
        l.set_prop(LayerField::Name, LayerPropValue::Name("Top".into()))
            .expect("valid");
        assert_eq!(l.name, "Top");
        l.set_prop(LayerField::Visible, LayerPropValue::Visible(false))
            .expect("valid");
        l.set_prop(LayerField::Locked, LayerPropValue::Locked(true))
            .expect("valid");
        l.set_prop(LayerField::Opacity, LayerPropValue::Opacity(0.25))
            .expect("valid");
        l.set_prop(LayerField::Blend, LayerPropValue::Blend("multiply"))
            .expect("valid");
        assert!(!l.visible);
        assert!(l.locked);
        assert_eq!(l.opacity, 0.25);
        assert_eq!(l.blend, "multiply");
        assert!(
            l.set_prop(LayerField::Opacity, LayerPropValue::Opacity(9.0))
                .is_err()
        );
        assert!(
            l.set_prop(LayerField::Blend, LayerPropValue::Blend("nope"))
                .is_err()
        );
        assert!(
            l.set_prop(
                LayerField::Name,
                LayerPropValue::Visible(true) // wrong tag
            )
            .is_err()
        );
    }

    #[test]
    fn as_source_carries_all_four_compositor_inputs() {
        let mut l = blank_layer("layer-0001");
        l.opacity = 0.5;
        l.blend = "screen";
        l.visible = false;
        let src = l.as_source();
        assert!(src.store.is_some());
        assert_eq!(src.opacity, 0.5);
        assert_eq!(src.mode, "screen");
        assert!(!src.is_visible());
        assert!(!src.is_transparent(), "0.5 still contributes");
        l.opacity = 0.0;
        assert!(l.as_source().is_transparent());
        // A NaN opacity is `!(opacity > 0)` in JS — hiding, not a crash.
        l.opacity = f64::NAN;
        assert!(l.as_source().is_transparent());
    }

    #[test]
    fn layer_field_round_trips_through_the_js_strings() {
        for s in LAYER_PROP_FIELDS {
            assert_eq!(LayerField::parse(s).expect("known").as_str(), s);
        }
        assert!(LayerField::parse("nope").is_err());
    }
}
