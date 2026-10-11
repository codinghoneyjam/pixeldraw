//! `fillet_polygon` — corner rounding, the geometry input to the fill masks.

use super::scanline::py_round;

/// Fillet polygon corners — exact port of the legacy `geometry.py fillet_polygon`.
///
/// `steps` arc samples per corner; the default 8 is what the fixtures used.
pub fn fillet_polygon(pts: &[[i32; 2]], radius: f64, steps: usize) -> Vec<[i32; 2]> {
    if radius <= 0.0 || pts.len() < 3 {
        return pts.to_vec();
    }
    let local: Vec<[i32; 2]> = pts.to_vec();
    let n = local.len();
    let mut new_pts: Vec<[i32; 2]> = Vec::new();
    for i in 0..n {
        let p_prev = local[(i + n - 1) % n];
        let p_curr = local[i];
        let p_next = local[(i + 1) % n];
        let v1x = (p_prev[0] - p_curr[0]) as f64;
        let v1y = (p_prev[1] - p_curr[1]) as f64;
        let v2x = (p_next[0] - p_curr[0]) as f64;
        let v2y = (p_next[1] - p_curr[1]) as f64;
        let l1 = v1x.hypot(v1y);
        let l2 = v2x.hypot(v2y);
        if l1 == 0.0 || l2 == 0.0 {
            new_pts.push(p_curr);
            continue;
        }
        let (u1x, u1y) = (v1x / l1, v1y / l1);
        let (u2x, u2y) = (v2x / l2, v2y / l2);
        let dot = (u1x * u2x + u1y * u2y).clamp(-0.999, 0.999);
        let half_angle = dot.acos() / 2.0;
        let tan_half = half_angle.tan();
        if tan_half < 0.001 {
            new_pts.push(p_curr);
            continue;
        }
        let max_r = radius.min(l1 * 0.45 * tan_half).min(l2 * 0.45 * tan_half);
        let tangent_len = max_r / tan_half;
        let p_start_x = p_curr[0] as f64 + u1x * tangent_len;
        let p_start_y = p_curr[1] as f64 + u1y * tangent_len;
        let p_end_x = p_curr[0] as f64 + u2x * tangent_len;
        let p_end_y = p_curr[1] as f64 + u2y * tangent_len;
        let bis_x = u1x + u2x;
        let bis_y = u1y + u2y;
        let b_len = bis_x.hypot(bis_y);
        if b_len < 0.001 {
            new_pts.push(p_curr);
            continue;
        }
        let (w_x, w_y) = (bis_x / b_len, bis_y / b_len);
        let center_dist = max_r / half_angle.sin();
        let cx = p_curr[0] as f64 + w_x * center_dist;
        let cy = p_curr[1] as f64 + w_y * center_dist;

        let a_start = (p_start_y - cy).atan2(p_start_x - cx);
        let a_end = (p_end_y - cy).atan2(p_end_x - cx);

        let diff = ((a_end - a_start + std::f64::consts::PI + 2.0 * std::f64::consts::PI)
            % (2.0 * std::f64::consts::PI))
            - std::f64::consts::PI;
        for s in 0..=steps {
            let ang = a_start + diff * (s as f64 / steps as f64);
            new_pts.push([
                py_round(cx + max_r * ang.cos()) as i32,
                py_round(cy + max_r * ang.sin()) as i32,
            ]);
        }
    }
    new_pts
}
