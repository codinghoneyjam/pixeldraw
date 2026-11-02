//! Recipe command generation for pantograph profiles.

use super::*;

pub(super) fn pantograph_cmds(profile: &Value, layers: &[Value]) -> Vec<Value> {
    let g = &profile["geometry"];
    let c = &profile["colors"];
    let mut out = Vec::new();
    let mk = |b: &Value, r: i32, fill: &Value, outline: &Value, width: Value| {
        let mut v = json!({"cmd":"rounded_rect","box":[[b[0],b[1]],[b[2],b[3]]],"radius":r,"fill":fill,"outline":outline});
        if !width.is_null() {
            v["width"] = width
        }
        v
    };
    for l in layers {
        if let Some(k) = l.as_str() {
            let pair = match k {
                "outer_frame" => Some((
                    &g["outer_frame"],
                    "bbox",
                    "outer_frame_bg_hex",
                    "outer_frame_border_hex",
                )),
                "socket_well" => Some((&g["socket_well"], "bbox", "socket_well_bg_hex", "")),
                "inner_keycap_normal" => Some((
                    &g["inner_keycap"],
                    "normal_bbox",
                    "keycap_body_normal_hex",
                    "keycap_border_normal_hex",
                )),
                "inner_keycap_pressed" => Some((
                    &g["inner_keycap"],
                    "pressed_bbox",
                    "keycap_body_pressed_hex",
                    "keycap_border_pressed_hex",
                )),
                "inner_keycap_hover" => Some((
                    &g["inner_keycap"],
                    "normal_bbox",
                    "keycap_body_hover_hex",
                    "keycap_border_hover_hex",
                )),
                "inner_keycap_disabled" => Some((
                    &g["inner_keycap"],
                    "normal_bbox",
                    "keycap_body_disabled_hex",
                    "keycap_border_disabled_hex",
                )),
                "visor_window_normal" | "visor_window_pressed" => Some((
                    &g["visor_window"],
                    if layers.iter().any(|x| x == "visor_window_pressed") {
                        "pressed_bbox"
                    } else {
                        "normal_bbox"
                    },
                    "visor_bg_hex",
                    "visor_border_hex",
                )),
                _ => None,
            };
            if let Some((spec, bn, f, ol)) = pair {
                out.push(mk(
                    &spec[bn],
                    u(&spec["corner_radius"], 0) as i32,
                    &c[f],
                    if ol.is_empty() { &Value::Null } else { &c[ol] },
                    json!(
                        spec.get("border_width")
                            .or(spec.get("borderwidth"))
                            .map(|v| u(v, 1))
                    ),
                ));
            }
        }
    }
    out
}
