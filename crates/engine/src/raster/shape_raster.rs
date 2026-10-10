//! Apply a `ShapeSpec` through a pixel writer — ported from
//! `src/core/raster/shape_raster.js`.
//!
//! This is the entry point every shape path shares: the pen tools, the shape
//! tool's commit, the HUD brackets and the visor fill all come through here.
//! It composes the already-verified mask primitives ([`ellipse_mask`],
//! [`rrect_mask`], [`outline_ring`], [`line_mask`], [`polygon_mask`]) and writes
//! the result, so its parity is inherited from theirs plus the write order.

use crate::errors::{DrawToolError, EngineResult};
use crate::raster::PixelSink;
use crate::raster::mask::{assert_mask_size, ellipse_mask, outline_ring, rrect_mask};
use crate::raster::polygon::{
    polygon_bbox, polygon_inset_ring, polygon_mask, polygon_outline_mask,
};
use crate::raster::segment::line_mask;

/// What the shape kind is, mirroring the JS `spec.kind` strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Rect,
    Rrect,
    Ellipse,
    Polygon,
}

impl ShapeKind {
    pub fn parse(s: &str) -> EngineResult<Self> {
        Ok(match s {
            "line" => Self::Line,
            "rect" => Self::Rect,
            "rrect" => Self::Rrect,
            "ellipse" => Self::Ellipse,
            "polygon" => Self::Polygon,
            other => {
                return Err(DrawToolError::out_of_range(format!(
                    "unknown shape kind {other}"
                )));
            }
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Rect => "rect",
            Self::Rrect => "rrect",
            Self::Ellipse => "ellipse",
            Self::Polygon => "polygon",
        }
    }
}

/// Fill mode, mirroring `spec.fillMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    Outline,
    Fill,
    Both,
}

impl FillMode {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("fill") => Self::Fill,
            Some("both") => Self::Both,
            _ => Self::Outline,
        }
    }
}

/// A shape specification as the tools build it.
#[derive(Debug, Clone)]
pub struct ShapeSpec {
    pub kind: ShapeKind,
    pub stroke_width: i32,
    pub fill_mode: FillMode,
    /// `[x, y, w, h]`, for the boxed kinds.
    pub bbox: Option<[i32; 4]>,
    /// Endpoints, for `line`.
    pub p0: Option<(i32, i32)>,
    pub p1: Option<(i32, i32)>,
    /// Corner radius, for `rrect`.
    pub radius: i32,
    /// Vertices, for `polygon`.
    pub points: Vec<[i32; 2]>,
}

impl ShapeSpec {
    /// `assertBrushSize` — integer 1..=64.
    pub fn check_stroke_width(sw: i32) -> EngineResult<()> {
        use crate::constants::{PEN_MAX, PEN_MIN};
        if !(PEN_MIN as i32..=PEN_MAX as i32).contains(&sw) {
            return Err(DrawToolError::out_of_range(format!(
                "brush size {sw} out of range"
            )));
        }
        Ok(())
    }

    /// Stroke width with the JS default: `undefined` → 1, truncated to int.
    pub fn stroke_width_or_default(v: Option<f64>) -> EngineResult<i32> {
        let sw = match v {
            None => 1,
            Some(f) => f.trunc() as i32,
        };
        Self::check_stroke_width(sw)?;
        Ok(sw)
    }
}

/// Write every set cell of a mask at an offset.
///
/// No coordinate clipping of its own — `PixelSink::set` drops out-of-canvas.
fn write_mask<S: PixelSink>(
    sink: &mut S,
    mask: &crate::raster::Mask,
    dx: i32,
    dy: i32,
    packed: u32,
) -> usize {
    let mut count = 0;
    for y in 0..mask.h {
        for x in 0..mask.w {
            if mask.is_set(x, y) {
                sink.set(dx + x as i32, dy + y as i32, packed);
                count += 1;
            }
        }
    }
    count
}

/// Rasterize a shape spec, returning the number of pixels written.
pub fn apply_shape<S: PixelSink>(
    sink: &mut S,
    spec: &ShapeSpec,
    primary: u32,
    secondary: u32,
) -> EngineResult<usize> {
    let sw = spec.stroke_width;
    ShapeSpec::check_stroke_width(sw)?;

    if spec.kind == ShapeKind::Line {
        let (p0, p1) = match (spec.p0, spec.p1) {
            (Some(a), Some(b)) => (a, b),
            _ => return Err(DrawToolError::out_of_range("line needs p0 and p1")),
        };
        let m = line_mask((p0.0, p0.1), (p1.0, p1.1), sw)?;
        return Ok(write_mask(sink, &m, m.x, m.y, primary));
    }

    if spec.kind == ShapeKind::Polygon {
        let pts = &spec.points;
        let bb = polygon_bbox(pts)?;
        let local: Vec<[i32; 2]> = pts.iter().map(|p| [p[0] - bb[0], p[1] - bb[1]]).collect();
        let pfill = polygon_mask(&local, bb[2] as usize, bb[3] as usize)?;
        if spec.fill_mode == FillMode::Fill {
            return Ok(write_mask(sink, &pfill, bb[0], bb[1], primary));
        }
        let pring = if sw >= 2 {
            polygon_inset_ring(&pfill, sw)
        } else {
            polygon_outline_mask(&local, bb[2].max(bb[3]) as usize, sw)?
        };
        if spec.fill_mode == FillMode::Both {
            write_mask(sink, &pfill, bb[0], bb[1], secondary);
        }
        return Ok(write_mask(sink, &pring, bb[0], bb[1], primary));
    }

    let [bx, by, bw, bh] = spec
        .bbox
        .ok_or_else(|| DrawToolError::out_of_range("boxed shapes need a bbox"))?;
    assert_mask_size(bw as usize, bh as usize)?;
    let radius = if spec.kind == ShapeKind::Rrect {
        spec.radius
    } else {
        0
    };
    let outer = if spec.kind == ShapeKind::Ellipse {
        ellipse_mask(bw as usize, bh as usize)?
    } else {
        rrect_mask(bw as usize, bh as usize, radius as i64)?
    };
    if spec.fill_mode == FillMode::Fill {
        return Ok(write_mask(sink, &outer, bx, by, primary));
    }
    let kind_str = if spec.kind == ShapeKind::Ellipse {
        "ellipse"
    } else {
        "rrect"
    };
    let ring = outline_ring(
        kind_str,
        bw as usize,
        bh as usize,
        radius as i64,
        sw as usize,
    )?;
    if spec.fill_mode == FillMode::Both {
        write_mask(sink, &outer, bx, by, secondary);
    }
    Ok(write_mask(sink, &ring, bx, by, primary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Rec {
        cells: Vec<(i32, i32, u32)>,
    }

    impl PixelSink for Rec {
        fn set(&mut self, x: i32, y: i32, p: u32) {
            self.cells.push((x, y, p));
        }

        fn get(&self, _x: i32, _y: i32) -> u32 {
            0
        }
    }

    fn spec(kind: ShapeKind) -> ShapeSpec {
        ShapeSpec {
            kind,
            stroke_width: 1,
            fill_mode: FillMode::Outline,
            bbox: Some([0, 0, 8, 8]),
            p0: Some((0, 0)),
            p1: Some((4, 4)),
            radius: 0,
            points: vec![],
        }
    }

    #[test]
    fn stroke_width_defaults_and_guards() {
        assert_eq!(ShapeSpec::stroke_width_or_default(None).unwrap(), 1);
        assert_eq!(ShapeSpec::stroke_width_or_default(Some(3.7)).unwrap(), 3);
        assert!(ShapeSpec::stroke_width_or_default(Some(0.0)).is_err());
        assert!(ShapeSpec::stroke_width_or_default(Some(65.0)).is_err());
    }

    #[test]
    fn fill_mode_parses_like_the_js_default() {
        assert_eq!(FillMode::parse(None), FillMode::Outline);
        assert_eq!(FillMode::parse(Some("outline")), FillMode::Outline);
        assert_eq!(FillMode::parse(Some("fill")), FillMode::Fill);
        assert_eq!(FillMode::parse(Some("both")), FillMode::Both);
    }

    #[test]
    fn kind_parse_accepts_the_tool_set() {
        for k in ["line", "rect", "rrect", "ellipse", "polygon"] {
            assert!(ShapeKind::parse(k).is_ok(), "{k}");
        }
        assert!(ShapeKind::parse("blob").is_err());
    }

    #[test]
    fn rect_fill_paints_every_cell() {
        let mut rec = Rec::default();
        let s = ShapeSpec {
            fill_mode: FillMode::Fill,
            ..spec(ShapeKind::Rrect)
        };
        let n = apply_shape(&mut rec, &s, 0xff0000ff, 0x00ff00ff).expect("valid");
        assert_eq!(n, 64);
        assert!(rec.cells.iter().all(|&(_, _, p)| p == 0xff0000ff));
    }

    #[test]
    fn both_mode_writes_secondary_then_primary() {
        let mut rec = Rec::default();
        let s = ShapeSpec {
            fill_mode: FillMode::Both,
            ..spec(ShapeKind::Ellipse)
        };
        apply_shape(&mut rec, &s, 0xff0000ff, 0x00ff00ff).expect("valid");
        // Primary (outline) is written after secondary (fill), so the last
        // pixel written per cell is the primary one.
        let last = rec.cells.last().expect("some cells");
        assert_eq!(last.2, 0xff0000ff);
        // Both colours appear: the ring overpaints the fill.
        assert!(rec.cells.iter().any(|&(_, _, p)| p == 0x00ff00ff));
        assert!(rec.cells.iter().any(|&(_, _, p)| p == 0xff0000ff));
    }

    #[test]
    fn polygon_outline_stays_inside_the_fill() {
        let mut rec = Rec::default();
        let s = ShapeSpec {
            kind: ShapeKind::Polygon,
            stroke_width: 1,
            fill_mode: FillMode::Outline,
            points: vec![[2, 2], [10, 2], [10, 10], [2, 10]],
            ..spec(ShapeKind::Polygon)
        };
        let n = apply_shape(&mut rec, &s, 0xff0000ff, 0).expect("valid");
        assert!(n > 0);
        // A width-1 outline of a 9x9 square is its perimeter: 32 cells.
        assert_eq!(n, 32);
    }

    #[test]
    fn line_spec_needs_both_endpoints() {
        let mut rec = Rec::default();
        let s = ShapeSpec {
            p1: None,
            ..spec(ShapeKind::Line)
        };
        assert!(apply_shape(&mut rec, &s, 0, 0).is_err());
    }

    #[test]
    fn bad_stroke_width_is_rejected_before_any_write() {
        let mut rec = Rec::default();
        let s = ShapeSpec {
            stroke_width: 99,
            ..spec(ShapeKind::Ellipse)
        };
        assert!(apply_shape(&mut rec, &s, 0, 0).is_err());
        assert!(rec.cells.is_empty());
    }
}
