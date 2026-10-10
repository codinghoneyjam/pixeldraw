//! Atlas sheet assembly — PIL's `paste` law, ported from `src/core/raster/atlas.js`.
//!
//! The legacy baker assembles the 16-slot sheet with
//! `sheet.paste(part, (idx * CANVAS, 0), part)` — each tile pasted through
//! **itself** as the mask. That squares alpha and premultiplies colour:
//!
//! ```text
//! outR = (c * a + 127) / 255      outA = (a * a + 127) / 255
//! ```
//!
//! **Plain source-over does not reproduce this** (alpha 128 → 64, not 128), so
//! [`paste_pixel`] must keep the closed form. Measured over all 256 alpha values
//! and 2000 random RGBA inputs. Tiles never overlap (disjoint columns) but the
//! loop honours the general `out = src*m + dst*(1-m)` form anyway.

use crate::errors::{DrawToolError, EngineResult};
use crate::pixel::Rgba;
use crate::raster::mask::assert_mask_size;

/// PIL's integer blend rounding: `(value * mask + 127) / 255`.
#[inline]
fn blend255(value: u32, mask: u32) -> u8 {
    ((value * mask + 127) / 255) as u8
}

/// PIL `paste` of an RGBA source through itself as the mask, one channel.
pub fn paste_channel(dst_c: u8, src_c: u8, a: u8) -> u8 {
    let inv = 255 - a as u32;
    blend255(src_c as u32, a as u32) + blend255(dst_c as u32, inv)
}

/// PIL `paste` of an RGBA source through itself as the mask, all four channels.
pub fn paste_pixel(dst: Rgba, src: Rgba) -> Rgba {
    let a = src[3];
    if a == 0 {
        return dst;
    }
    if dst[3] == 0 {
        return [
            blend255(src[0] as u32, a as u32),
            blend255(src[1] as u32, a as u32),
            blend255(src[2] as u32, a as u32),
            blend255(a as u32, a as u32),
        ];
    }
    [
        paste_channel(dst[0], src[0], a),
        paste_channel(dst[1], src[1], a),
        paste_channel(dst[2], src[2], a),
        paste_channel(dst[3], src[3], a),
    ]
}

/// One slot tile: packed RGBA per pixel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u32>,
}

/// An assembled RGBA strip, `{ w, h, data }` with `data` bytes row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl Sheet {
    /// The packed RGBA of one pixel.
    pub fn rgba(&self, x: usize, y: usize) -> Rgba {
        let o = (y * self.w + x) * 4;
        [
            self.data[o],
            self.data[o + 1],
            self.data[o + 2],
            self.data[o + 3],
        ]
    }
}

/// Assemble a horizontal atlas strip from slot tiles.
///
/// Tiles must all be the same size; the strip is `tiles.len() * tileW` wide.
pub fn assemble_sheet(tiles: &[Tile]) -> EngineResult<Sheet> {
    if tiles.is_empty() {
        return Err(DrawToolError::invalid_state(
            "assembleSheet needs at least one tile",
        ));
    }
    let tw = tiles[0].w;
    let th = tiles[0].h;
    // `assert_mask_size` rejects w/h < 1 as OUT_OF_RANGE, matching the JS comment
    // that the zero-size case is caught by the length check rather than here.
    assert_mask_size(tw, th)?;
    for t in tiles {
        if t.w != tw || t.h != th {
            return Err(DrawToolError::invalid_state(format!(
                "tile size mismatch: expected {tw}x{th}, got {}x{}",
                t.w, t.h
            )));
        }
        if t.data.len() != tw * th {
            return Err(DrawToolError::invalid_state(format!(
                "tile {}x{} data length {} != {}",
                t.w,
                t.h,
                t.data.len(),
                tw * th
            )));
        }
    }
    let mut out = Sheet {
        w: tw * tiles.len(),
        h: th,
        data: vec![0u8; tw * tiles.len() * th * 4],
    };
    for (slot, tile) in tiles.iter().enumerate() {
        let x_off = slot * tw;
        for y in 0..th {
            for x in 0..tw {
                let s = tile.data[y * tw + x];
                if s == 0 {
                    continue;
                }
                let d_off = (y * out.w + x_off + x) * 4;
                let dst = [
                    out.data[d_off],
                    out.data[d_off + 1],
                    out.data[d_off + 2],
                    out.data[d_off + 3],
                ];
                let src = [
                    (s & 0xff) as u8,
                    ((s >> 8) & 0xff) as u8,
                    ((s >> 16) & 0xff) as u8,
                    ((s >> 24) & 0xff) as u8,
                ];
                let merged = paste_pixel(dst, src);
                out.data[d_off..d_off + 4].copy_from_slice(&merged);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{group, usize_field};

    /// `"0A141EFF"` → packed u32 in the engine's little-endian layout.
    ///
    /// The fixture prints the four bytes in **RGBA order**, so `0A` is red and
    /// not the most-significant byte of a number.
    fn hex32(s: &str) -> u32 {
        assert_eq!(s.len(), 8, "one pixel is eight hex digits");
        let bytes: Vec<u8> = (0..4)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).expect("byte"))
            .collect();
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    // <META - ROLE : golden paste group, 5 cases | L150-185>
    #[test]
    fn golden_paste_cases() {
        for case in group("paste") {
            let cw = usize_field(case, "cell_w");
            let ch = usize_field(case, "cell_h");
            let slots = usize_field(case, "slots");
            let inputs: Vec<&str> = case["inputs"]
                .as_array()
                .expect("inputs")
                .iter()
                .map(|v| v.as_str().expect("input hex"))
                .collect();
            let expect: Vec<&str> = case["expect"]
                .as_array()
                .expect("expect")
                .iter()
                .map(|v| v.as_str().expect("expect hex"))
                .collect();

            let tiles: Vec<Tile> = inputs
                .iter()
                .map(|h| Tile {
                    w: cw,
                    h: ch,
                    data: vec![hex32(h); cw * ch],
                })
                .collect();
            assert_eq!(tiles.len(), slots);
            let sheet = assemble_sheet(&tiles).expect("valid tiles");

            assert_eq!(sheet.w, cw * slots);
            assert_eq!(sheet.h, ch);
            // Each expected hex string is one row of the strip.
            assert_eq!(expect.len(), ch, "{} rows", case["name"]);
            for (y, row) in expect.iter().enumerate() {
                let cells: Vec<Rgba> = (0..slots).map(|x| sheet.rgba(x, y)).collect();
                let joined = cells
                    .iter()
                    .map(|c| c.iter().map(|b| format!("{b:02X}")).collect::<String>())
                    .collect::<String>();
                assert_eq!(joined, *row, "{} row {y}", case["name"]);
            }
        }
    }

    #[test]
    fn paste_squares_alpha_as_measured() {
        // The whole point: alpha 128 must become 64 under PIL's law, not 128.
        assert_eq!(paste_channel(0, 128, 128), 64);
        assert_eq!(paste_pixel([0, 0, 0, 0], [255, 0, 0, 128])[3], 64);
    }

    #[test]
    fn transparent_source_is_a_no_op() {
        assert_eq!(
            paste_pixel([10, 20, 30, 200], [255, 255, 255, 0]),
            [10, 20, 30, 200]
        );
    }

    #[test]
    fn empty_destination_premultiplies_the_source() {
        let got = paste_pixel([0, 0, 0, 0], [200, 100, 50, 128]);
        assert_eq!(got[0], blend255(200, 128));
        assert_eq!(got[1], blend255(100, 128));
        assert_eq!(got[2], blend255(50, 128));
        assert_eq!(got[3], blend255(128, 128));
    }

    #[test]
    fn zero_packed_pixels_are_skipped() {
        let tiles = vec![Tile {
            w: 2,
            h: 1,
            data: vec![0, hex32("FF0000FF")],
        }];
        let sheet = assemble_sheet(&tiles).expect("valid");
        assert_eq!(sheet.rgba(0, 0), [0, 0, 0, 0]);
        assert_eq!(sheet.rgba(1, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn rejects_mismatched_and_empty_input() {
        let ok = Tile {
            w: 1,
            h: 1,
            data: vec![0],
        };
        let bad = Tile {
            w: 2,
            h: 1,
            data: vec![0, 0],
        };
        assert!(assemble_sheet(&[ok.clone(), bad]).is_err());
        assert!(assemble_sheet(&[]).is_err());
        assert!(
            assemble_sheet(&[Tile {
                w: 1,
                h: 1,
                data: vec![],
            }])
            .is_err()
        );
    }
}
