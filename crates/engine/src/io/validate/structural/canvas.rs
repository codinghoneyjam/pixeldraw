//! Canvas object validation.

use super::*;

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
