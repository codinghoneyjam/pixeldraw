//! The PIL `Draw.c` scanline engine — edge table, active-edge sort, span fill.
//!
//! Ported from `src/core/raster/polygon.js`. Every coordinate here is a C
//! `float` in the original: one `inter_x` helper reproduces the JS's triple
//! `Math.fround` intersection expression, which is what makes the port
//! bit-exact instead of merely close. Rationale: `docs/raster_internals.md` §2.

/// `Math.fround` — round an f64 to the nearest f32.
#[inline]
pub fn fround(v: f64) -> f32 {
    v as f32
}

/// `Math.fround(Math.fround((y - y0) * dx) + x0)` — the intersection abscissa.
///
/// The product of two f32 operands is exact in f64, so the inner f32 multiply
/// and an fround of the exact product agree bit for bit; the same argument
/// covers the outer `+ x0`, which is why that explicit fround is load-bearing.
#[inline]
pub fn inter_x(y: i32, e: &Edge) -> f32 {
    let inner = (y - e.y0) as f32 * e.dx;
    fround(inner as f64 + e.x0 as f64)
}

/// PIL's `roundUp`: half away from zero, toward +∞ for positives.
#[inline]
pub fn round_up(f: f32) -> i32 {
    if f >= 0.0 {
        (f + 0.5).floor() as i32
    } else {
        -((-f + 0.5).floor() as i32)
    }
}

/// PIL's `roundDown`.
#[inline]
pub fn round_down(f: f32) -> i32 {
    if f >= 0.0 {
        (f - 0.5).ceil() as i32
    } else {
        -((-f - 0.5).ceil() as i32)
    }
}

/// C `roundf`: half away from zero.
#[inline]
pub fn round_c(f: f32) -> i32 {
    if f >= 0.0 {
        (f + 0.5).floor() as i32
    } else {
        -((-f + 0.5).floor() as i32)
    }
}

/// Python `round()`: half to even.
#[inline]
pub fn py_round(v: f64) -> i64 {
    let f = v.floor();
    let r = v - f;
    if r > 0.5 {
        (f + 1.0) as i64
    } else if r < 0.5 {
        f as i64
    } else if (f as i64) % 2 == 0 {
        // Exactly .5 → Python rounds to even, which keeps `f`.
        f as i64
    } else {
        (f + 1.0) as i64
    }
}

/// One polygon edge. `dx` is the f32 slope `(x1-x0)/(y1-y0)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub xmin: i32,
    pub xmax: i32,
    pub ymin: i32,
    pub ymax: i32,
    pub d: i32,
    pub dx: f32,
    pub x0: i32,
    pub y0: i32,
}

/// `polyAddEdge` — fill the extent fields and the slope.
pub fn poly_add_edge(mut e: Edge, x0: i32, y0: i32, x1: i32, y1: i32) -> Edge {
    if x0 <= x1 {
        e.xmin = x0;
        e.xmax = x1;
    } else {
        e.xmin = x1;
        e.xmax = x0;
    }
    if y0 <= y1 {
        e.ymin = y0;
        e.ymax = y1;
    } else {
        e.ymin = y1;
        e.ymax = y0;
    }
    if y0 == y1 {
        e.d = 0;
        e.dx = 0.0;
    } else {
        // fround of the f64 quotient, matching JS exactly.
        e.dx = fround((x1 - x0) as f64 / (y1 - y0) as f64);
        e.d = if y0 == e.ymin { 1 } else { -1 };
    }
    e.x0 = x0;
    e.y0 = y0;
    e
}

/// Shared by `masks::wide_line_quad_edges`, which builds quads outside this file.
pub(crate) fn empty_edge() -> Edge {
    Edge {
        xmin: 0,
        xmax: 0,
        ymin: 0,
        ymax: 0,
        d: 0,
        dx: 0.0,
        x0: 0,
        y0: 0,
    }
}

/// Edge build with the consecutive-horizontal-merge rule from `ImagingDrawPolygon`.
///
/// Runs of same-`y` vertices collapse into one wider horizontal edge, which is
/// what keeps a staircase top edge from being filled twice.
pub fn poly_build_edges(pts: &[[i32; 2]]) -> Vec<Edge> {
    let mut e: Vec<Edge> = Vec::new();
    for i in 0..pts.len().saturating_sub(1) {
        let [x0, y0] = pts[i];
        let [x1, y1] = pts[i + 1];
        if y0 == y1 && i != 0 && y0 == pts[i - 1][1] {
            let last = e.last_mut().expect("edge exists for a run");
            if x1 > x0 && x0 > pts[i - 1][0] {
                last.xmax = x1;
                continue;
            }
            if x1 < x0 && x0 < pts[i - 1][0] {
                last.xmin = x1;
                continue;
            }
        }
        e.push(poly_add_edge(empty_edge(), x0, y0, x1, y1));
    }
    if let (Some(last), Some(first)) = (pts.last(), pts.first())
        && (last[0] != first[0] || last[1] != first[1])
    {
        e.push(poly_add_edge(
            empty_edge(),
            last[0],
            last[1],
            first[0],
            first[1],
        ));
    }
    e
}

/// Clipped horizontal span write.
fn poly_hline(data: &mut [u8], w: usize, h: usize, mut x0: i32, y: i32, mut x1: i32, val: u8) {
    if y < 0 || y >= h as i32 {
        return;
    }
    if x0 < 0 {
        x0 = 0;
    } else if x0 >= w as i32 {
        return;
    }
    if x1 < 0 {
        return;
    } else if x1 >= w as i32 {
        x1 = w as i32 - 1;
    }
    if x0 <= x1 {
        let row = y as usize * w;
        for x in x0..=x1 {
            data[row + x as usize] = val;
        }
    }
}

/// `polygon_generic` — the scanline fill, ported line by line.
///
/// The vertex-degeneracy adjustment (the block that nudges `xx[j-1]` by ±1 when
/// two edges meet exactly on a scanline) is what makes a shared vertex not open
/// a gap, and it is the first thing a rewrite loses.
pub fn polygon_generic(data: &mut [u8], w: usize, h: usize, edges: &[Edge]) {
    let mut table: Vec<Edge> = Vec::new();
    let mut ymin = h as i32 - 1;
    let mut ymax = 0;
    for e in edges {
        if e.ymin == e.ymax {
            poly_hline(data, w, h, e.xmin, e.ymin, e.xmax, 1);
        } else {
            table.push(*e);
        }
    }
    for e in edges {
        if ymin > e.ymin {
            ymin = e.ymin;
        }
        if ymax < e.ymax {
            ymax = e.ymax;
        }
    }
    if ymin < 0 {
        ymin = 0;
    }
    if ymax > h as i32 {
        ymax = h as i32;
    }
    let mut xx = vec![0f32; table.len() * 2];
    for y in ymin..=ymax {
        let mut j = 0usize;
        for i in 0..table.len() {
            let cur = table[i];
            if y >= cur.ymin && y <= cur.ymax {
                // fround(fround(fround(y - y0) * dx) + x0)
                let inter = inter_x(y, &cur);
                xx[j] = inter;
                j += 1;
                if y == cur.ymax && y < ymax {
                    xx[j] = xx[j - 1];
                    j += 1;
                } else if (y == cur.ymin || y == cur.ymax) && cur.dx != 0.0 {
                    for other in table.iter().take(i) {
                        if (y != other.ymin && y != other.ymax) || other.dx == 0.0 {
                            continue;
                        }
                        if round_c(xx[j - 1]) == round_c(inter_x(y, other)) {
                            let offset = if y == cur.ymax { -1 } else { 1 };
                            let adj = inter_x(y + offset, &cur);
                            if y + offset >= other.ymin && y + offset <= other.ymax {
                                let adj_o = inter_x(y + offset, other);
                                if xx[j - 1] > adj + 1.0 && xx[j - 1] > adj_o + 1.0 {
                                    xx[j - 1] = round_c(adj.max(adj_o)) as f32 + 1.0;
                                } else if xx[j - 1] < adj - 1.0 && xx[j - 1] < adj_o - 1.0 {
                                    xx[j - 1] = round_c(adj.min(adj_o)) as f32 - 1.0;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
        let mut vals: Vec<f32> = xx[..j].to_vec();
        vals.sort_unstable_by(|a, b| a.total_cmp(b));
        let mut i = 1;
        while i < j {
            poly_hline(data, w, h, round_up(vals[i - 1]), y, round_down(vals[i]), 1);
            i += 2;
        }
    }
}
