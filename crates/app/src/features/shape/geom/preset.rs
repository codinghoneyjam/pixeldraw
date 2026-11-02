//! Shape presets (F04): heart, N-pointed star, etc.
//!
//! Presets are **geometry generators** only — they produce a `Vec<Pt>` that
//! flows through the existing polygon raster path (`ShapeKind::Polygon`).
//! No new rasterizer.

use super::types::{Bbox, Pt, ShapePreset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetParams {
    pub rotation_deg: i32,
    pub samples: u8,
}

impl Default for PresetParams {
    fn default() -> Self {
        Self {
            rotation_deg: 0,
            samples: 32,
        }
    }
}

pub fn preset_points(preset: ShapePreset, bbox: Bbox, params: PresetParams) -> Vec<Pt> {
    match preset {
        ShapePreset::Heart => heart_points(bbox, params.samples),
        ShapePreset::Star {
            points,
            inner_ratio,
        } => star_points(
            bbox,
            points.max(3),
            inner_ratio.clamp(1, 99),
            params.rotation_deg,
        ),
    }
}

fn heart_points(bbox: Bbox, samples: u8) -> Vec<Pt> {
    let n = samples.clamp(8, 128) as usize;
    let cx = bbox.x as f64 + bbox.w as f64 / 2.0;
    let cy = bbox.y as f64 + bbox.h as f64 / 2.0;
    let rx = (bbox.w as f64 / 2.0).max(1.0);
    let ry = (bbox.h as f64 / 2.0).max(1.0);
    let mut pts = Vec::with_capacity(n);
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..n {
        let t = (i as f64 / n as f64) * std::f64::consts::TAU;
        // classic heart parametric
        let x = 16.0 * t.sin().powi(3);
        let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
        // normalize to [-1,1] roughly: x in [-16,16], y in [-17,13]
        let nx = x / 16.0;
        let ny = -y / 17.0; // flip y (canvas down)
        let px = (cx + nx * rx).round() as i32;
        let py = (cy + ny * ry).round() as i32;
        if seen.insert((px, py)) {
            pts.push((px, py));
        }
    }
    if pts.len() >= 3 && pts.first() != pts.last() {
        pts.push(pts[0]);
    }
    pts
}

fn star_points(bbox: Bbox, n: u8, inner_ratio: u8, rot_deg: i32) -> Vec<Pt> {
    let cx = bbox.x as f64 + bbox.w as f64 / 2.0;
    let cy = bbox.y as f64 + bbox.h as f64 / 2.0;
    let ro = (bbox.w.min(bbox.h) as f64 / 2.0).max(1.0);
    let ri = ro * (inner_ratio as f64 / 100.0);
    let rot = (rot_deg as f64).to_radians();
    let step = std::f64::consts::PI / n as f64;
    let mut pts = Vec::with_capacity((n as usize) * 2);
    for i in 0..(n as usize * 2) {
        let angle = rot + i as f64 * step - std::f64::consts::FRAC_PI_2;
        let r = if i % 2 == 0 { ro } else { ri };
        let px = (cx + r * angle.cos()).round() as i32;
        let py = (cy + r * angle.sin()).round() as i32;
        pts.push((px, py));
    }
    // close
    if !pts.is_empty() && pts.first() != pts.last() {
        pts.push(pts[0]);
    }
    pts
}
