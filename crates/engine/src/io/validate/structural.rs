//! Structural schema checks — canvas, chunk, raster, layer
//! (JS `io/validate_structural.js`) — plus the error
//! collector and report types every check shares.
//!
//! JS parity traps (UTF-16 lengths, `Number.isInteger` over
//! JSON, the hand-rolled PNG/BG matchers):
//! `docs/io_internals.md`.

use crate::blend::BLEND_MODES;
use crate::constants::{
    CHUNK_PX, MAX_CHUNKS_X, MAX_CHUNKS_Y, MAX_H_PX, MAX_W_PX, MIN_SIZE_PX, TILE_PX, UNIT_PX,
};
use crate::errors::CODE_SCHEMA;
use serde_json::Value;

// Schema-shape limits that live only in the JS validator, not in
// `constants` (which pins engine geometry): the error cap and the
pub(crate) const MAX_ERRORS: usize = 50;
const MAX_LAYER_ID_CHARS: usize = 64;
const MAX_NAME_CHARS: usize = 128;

/// One chunk cell holds at most one chunk entry — derived,
/// never restated.
const MAX_CHUNK_CELLS: u32 = MAX_CHUNKS_X * MAX_CHUNKS_Y;

const CANVAS_KEYS: [&str; 6] = [
    "tile_px",
    "unit_px",
    "width_px",
    "height_px",
    "background",
    "viewport",
];
const VIEWPORT_KEYS: [&str; 4] = ["x", "y", "w", "h"];
const LAYER_KEYS: [&str; 9] = [
    "layer_id", "name", "type", "visible", "locked", "opacity", "blend", "raster", "shapes",
];
const RASTER_KEYS: [&str; 3] = ["chunk_px", "encoding", "chunks"];
const CHUNK_KEYS: [&str; 3] = ["cx", "cy", "png"];

/// One collected violation — the JS `{code, path, message}` triple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

/// `{ok, errors}` — validation never throws, it only collects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<ValidationError>,
}

/// The error collector with the 50 cap (JS `collector.push()`).
pub struct Collector {
    errors: Vec<ValidationError>,
}

impl Collector {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub(crate) fn len(&self) -> usize {
        self.errors.len()
    }
    pub fn push(&mut self, code: &'static str, path: String, message: String) {
        if self.errors.len() < MAX_ERRORS {
            self.errors.push(ValidationError {
                code,
                path,
                message,
            });
        }
    }

    pub fn report(self) -> ValidationReport {
        let ok = self.errors.is_empty();
        ValidationReport {
            ok,
            errors: self.errors,
        }
    }
}

/// What [`check_canvas`] returns, or `None` when either dim
/// failed (JS returns `null`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasInfo {
    pub width_px: u32,
    pub height_px: u32,
    pub viewport: Option<Viewport>,
}

/// A viewport rect. Its size is deliberately NOT a multiple of
/// 32 — expressing a non-aligned export size is the whole
/// point of the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewport {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

/// What [`check_layer`] returns. The JS also carries the
/// layer `type`, but every caller re-reads it off the layer
/// itself, so it is dropped here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerInfo {
    pub structural_chunks: Vec<bool>,
}

pub(crate) fn is_obj(v: &Value) -> bool {
    matches!(v, Value::Object(_))
}

/// Flag every key outside the whitelist as SCHEMA (JS
/// `checkNoExtra`).
pub(crate) fn check_no_extra(obj: &Value, allowed: &[&str], base: &str, collector: &mut Collector) {
    if let Some(map) = obj.as_object() {
        for k in map.keys() {
            if !allowed.contains(&k.as_str()) {
                collector.push(
                    CODE_SCHEMA,
                    format!("{base}/{k}"),
                    format!("unexpected property {k}"),
                );
            }
        }
    }
}

/// `Number.isInteger` over a JSON value. serde_json preserves
/// the text's integer/float split (`32` → u64, `32.0` → f64),
/// so only integer-typed numbers pass — the deliberate choice
/// is documented in docs/io_internals.md.
fn int_of(v: Option<&Value>) -> Option<i128> {
    match v {
        Some(Value::Number(n)) if n.is_i64() => n.as_i64().map(i128::from),
        Some(Value::Number(n)) if n.is_u64() => Some(n.as_u64()? as i128),
        _ => None,
    }
}

/// `PNG_RE` — the base64 of any PNG starts with `iVBORw0KGgo`
/// (the 8-byte signature's first 11 base64 chars). Hand-rolled
/// because the engine has no regex dep; the equivalence argument
/// is in docs/io_internals.md.
fn is_png_b64(s: &str) -> bool {
    let Some(body) = s.strip_prefix("iVBORw0KGgo") else {
        return false;
    };
    // `[A-Za-z0-9+/]*={0,2}$`: strip at most two trailing `=`,
    // then require every remaining byte in the alphabet — an
    let unpadded = body
        .strip_suffix("==")
        .or_else(|| body.strip_suffix("="))
        .unwrap_or(body);
    unpadded
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

/// `BG_RE` — `transparent` or `#rrggbb`/`#rrggbbaa`.
fn is_background(s: &str) -> bool {
    if s == "transparent" {
        return true;
    }
    match s.strip_prefix('#') {
        Some(hex) => matches!(hex.len(), 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit()),
        None => false,
    }
}

/// Validate a canvas object. Returns the dims (plus viewport)
/// or `None` when either dim failed — the caller then skips
/// every check that needs the canvas size (JS returns `null`).
pub fn check_canvas(c: &Value, base: &str, collector: &mut Collector) -> Option<CanvasInfo> {
    if !is_obj(c) {
        collector.push(
            CODE_SCHEMA,
            base.to_string(),
            "canvas must be an object".to_string(),
        );
        return None;
    }
    check_no_extra(c, &CANVAS_KEYS, base, collector);
    // `!== TILE_PX` compares by value, so a JSON `64.0`
    // passes — `as_f64` mirrors that, `as_u64` would not.
    if c.get("tile_px").and_then(Value::as_f64) != Some(f64::from(TILE_PX)) {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/tile_px"),
            "tile_px must be 64".to_string(),
        );
    }
    if c.get("unit_px").and_then(Value::as_f64) != Some(f64::from(UNIT_PX)) {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/unit_px"),
            "unit_px must be 32".to_string(),
        );
    }
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    if let Some(w) = int_of(c.get("width_px")).filter(|w| {
        (i128::from(MIN_SIZE_PX)..=i128::from(MAX_W_PX)).contains(w)
            && *w % i128::from(UNIT_PX) == 0
    }) {
        width = Some(w as u32);
    } else {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/width_px"),
            format!("width_px must be a multiple of {UNIT_PX} in [{MIN_SIZE_PX},{MAX_W_PX}]"),
        );
    }
    if let Some(h) = int_of(c.get("height_px")).filter(|h| {
        (i128::from(MIN_SIZE_PX)..=i128::from(MAX_H_PX)).contains(h)
            && *h % i128::from(UNIT_PX) == 0
    }) {
        height = Some(h as u32);
    } else {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/height_px"),
            format!("height_px must be a multiple of {UNIT_PX} in [{MIN_SIZE_PX},{MAX_H_PX}]"),
        );
    }
    if let Some(bg) = c.get("background") {
        let bad = match bg.as_str() {
            Some(s) => !is_background(s),
            None => true,
        };
        if bad {
            collector.push(
                CODE_SCHEMA,
                format!("{base}/background"),
                "background must be transparent or #rrggbb[#aa]".to_string(),
            );
        }
    }
    // Viewport is optional and `null` counts as absent. When
    // present it must be an integer rect fully inside the
    let mut viewport = None;
    if let Some(vp) = c.get("viewport").filter(|v| !v.is_null()) {
        let vp_base = format!("{base}/viewport");
        if !is_obj(vp) {
            collector.push(
                CODE_SCHEMA,
                vp_base.clone(),
                "viewport must be an object".to_string(),
            );
        } else {
            check_no_extra(vp, &VIEWPORT_KEYS, &vp_base, collector);
            let mut rect = [0i128; 4];
            let mut all_ints = true;
            for (slot, key) in rect.iter_mut().zip(["x", "y", "w", "h"]) {
                match int_of(vp.get(key)) {
                    Some(v) => *slot = v,
                    None => all_ints = false,
                }
            }
            if !all_ints {
                collector.push(
                    CODE_SCHEMA,
                    vp_base.clone(),
                    "viewport x/y/w/h must all be integers".to_string(),
                );
            } else {
                let [x, y, w, h] = rect;
                if w < 1 || h < 1 {
                    collector.push(
                        CODE_SCHEMA,
                        vp_base.clone(),
                        "viewport w/h must be positive".to_string(),
                    );
                } else if let (Some(cw), Some(ch)) = (width, height)
                    && (x < 0 || y < 0 || x + w > i128::from(cw) || y + h > i128::from(ch))
                {
                    collector.push(
                        CODE_SCHEMA,
                        vp_base.clone(),
                        format!("viewport {x},{y} {w}x{h} exceeds canvas {cw}x{ch}"),
                    );
                } else {
                    viewport = Some(Viewport {
                        x: x as i64,
                        y: y as i64,
                        w: w as i64,
                        h: h as i64,
                    });
                }
            }
        }
    }
    match (width, height) {
        (Some(w), Some(h)) => Some(CanvasInfo {
            width_px: w,
            height_px: h,
            viewport,
        }),
        _ => None,
    }
}

/// Validate one chunk entry. `true` when cx/cy/png are all
/// sound — the semantic pass only decodes chunks that pass
/// this, but its duplicate and bounds checks run regardless.
pub fn check_chunk(ch: &Value, base: &str, collector: &mut Collector) -> bool {
    if !is_obj(ch) {
        collector.push(
            CODE_SCHEMA,
            base.to_string(),
            "chunk must be an object".to_string(),
        );
        return false;
    }
    check_no_extra(ch, &CHUNK_KEYS, base, collector);
    let mut ok = true;
    match int_of(ch.get("cx")) {
        Some(cx) if (0..i128::from(MAX_CHUNKS_X)).contains(&cx) => {}
        _ => {
            collector.push(
                CODE_SCHEMA,
                format!("{base}/cx"),
                format!("cx must be an integer in [0,{}]", MAX_CHUNKS_X - 1),
            );
            ok = false;
        }
    }
    match int_of(ch.get("cy")) {
        Some(cy) if (0..i128::from(MAX_CHUNKS_Y)).contains(&cy) => {}
        _ => {
            collector.push(
                CODE_SCHEMA,
                format!("{base}/cy"),
                format!("cy must be an integer in [0,{}]", MAX_CHUNKS_Y - 1),
            );
            ok = false;
        }
    }
    let png_ok = match ch.get("png").and_then(Value::as_str) {
        Some(s) => is_png_b64(s),
        None => false,
    };
    if !png_ok {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/png"),
            "png must be base64 starting with iVBORw0KGgo".to_string(),
        );
        ok = false;
    }
    ok
}

/// Validate a raster payload. Returns the per-chunk structural
/// flags (empty when `chunks` is missing or not an array).
pub fn check_raster(r: &Value, base: &str, collector: &mut Collector) -> Vec<bool> {
    if !is_obj(r) {
        collector.push(
            CODE_SCHEMA,
            base.to_string(),
            "raster must be an object".to_string(),
        );
        return Vec::new();
    }
    check_no_extra(r, &RASTER_KEYS, base, collector);
    if r.get("chunk_px").and_then(Value::as_f64) != Some(f64::from(CHUNK_PX)) {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/chunk_px"),
            format!("chunk_px must be {CHUNK_PX}"),
        );
    }
    if r.get("encoding").and_then(Value::as_str) != Some("png_base64") {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/encoding"),
            "encoding must be png_base64".to_string(),
        );
    }
    let Some(chunks) = r.get("chunks").and_then(Value::as_array) else {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/chunks"),
            "chunks must be an array".to_string(),
        );
        return Vec::new();
    };
    if chunks.len() > MAX_CHUNK_CELLS as usize {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/chunks"),
            format!("chunks must hold at most {MAX_CHUNK_CELLS} entries"),
        );
    }
    chunks
        .iter()
        .enumerate()
        .map(|(j, ch)| check_chunk(ch, &format!("{base}/chunks/{j}"), collector))
        .collect()
}

/// Validate one layer. `shapes` and `raster` are mutually
/// exclusive by type: a raster layer must not carry `shapes`,
/// a vector layer must not carry `raster`.
#[allow(clippy::collapsible_if)]
pub fn check_layer(ly: &Value, base: &str, collector: &mut Collector) -> LayerInfo {
    if !is_obj(ly) {
        collector.push(
            CODE_SCHEMA,
            base.to_string(),
            "layer must be an object".to_string(),
        );
        return LayerInfo {
            structural_chunks: Vec::new(),
        };
    }
    check_no_extra(ly, &LAYER_KEYS, base, collector);
    // JS `.length` counts UTF-16 code units, so a non-BMP
    // character counts double — `encode_utf16().count()` is
    let id_ok = match ly.get("layer_id").and_then(Value::as_str) {
        Some(s) => {
            let n = s.encode_utf16().count();
            (1..=MAX_LAYER_ID_CHARS).contains(&n)
        }
        None => false,
    };
    if !id_ok {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/layer_id"),
            "layer_id must be a string of 1-64 chars".to_string(),
        );
    }
    if let Some(name) = ly.get("name").and_then(Value::as_str) {
        if name.encode_utf16().count() > MAX_NAME_CHARS {
            collector.push(
                CODE_SCHEMA,
                format!("{base}/name"),
                "name must be a string of at most 128 chars".to_string(),
            );
        }
    }
    let layer_type = ly.get("type").and_then(Value::as_str);
    if layer_type != Some("raster") && layer_type != Some("vector") {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/type"),
            "type must be raster or vector".to_string(),
        );
    }
    if !matches!(ly.get("visible"), Some(Value::Bool(_))) {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/visible"),
            "visible must be boolean".to_string(),
        );
    }
    if !matches!(ly.get("locked"), Some(Value::Bool(_))) {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/locked"),
            "locked must be boolean".to_string(),
        );
    }
    let opacity_ok = match ly.get("opacity").and_then(Value::as_f64) {
        Some(o) => o.is_finite() && (0.0..=1.0).contains(&o),
        None => false,
    };
    if !opacity_ok {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/opacity"),
            "opacity must be a number in [0,1]".to_string(),
        );
    }
    let blend_ok = match ly.get("blend").and_then(Value::as_str) {
        Some(m) => BLEND_MODES.contains(&m),
        None => false,
    };
    if !blend_ok {
        collector.push(
            CODE_SCHEMA,
            format!("{base}/blend"),
            format!("blend must be one of: {}", BLEND_MODES.join(", ")),
        );
    }
    let mut structural_chunks = Vec::new();
    match layer_type {
        Some("raster") => {
            if ly.get("shapes").is_some() {
                collector.push(
                    CODE_SCHEMA,
                    format!("{base}/shapes"),
                    "raster layer must not have shapes".to_string(),
                );
            }
            match ly.get("raster") {
                None => collector.push(
                    CODE_SCHEMA,
                    format!("{base}/raster"),
                    "raster layer requires raster".to_string(),
                ),
                Some(r) => {
                    structural_chunks = check_raster(r, &format!("{base}/raster"), collector)
                }
            }
        }
        Some("vector") => {
            if ly.get("raster").is_some() {
                collector.push(
                    CODE_SCHEMA,
                    format!("{base}/raster"),
                    "vector layer must not have raster".to_string(),
                );
            }
            if !matches!(ly.get("shapes"), Some(Value::Array(_))) {
                collector.push(
                    CODE_SCHEMA,
                    format!("{base}/shapes"),
                    "vector layer requires shapes array".to_string(),
                );
            }
        }
        _ => {}
    }
    LayerInfo { structural_chunks }
}
