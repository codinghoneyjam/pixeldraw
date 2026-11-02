//! PNG row filters, colour unpacking, and Adam7 scan geometry.

use super::super::crc::unsupported;
use crate::errors::EngineResult;

#[derive(Debug, Clone, Copy)]
pub(super) struct Header {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) depth: u8,
    pub(super) color: u8,
    pub(super) interlace: u8,
}

impl Header {
    fn channels(&self) -> usize {
        match self.color {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => 0,
        }
    }

    fn bits_per_pixel(&self) -> usize {
        self.channels() * self.depth as usize
    }

    fn filter_bpp(&self) -> usize {
        self.bits_per_pixel().div_ceil(8).max(1)
    }

    fn row_bytes(&self, w: u32) -> usize {
        (w as usize * self.bits_per_pixel()).div_ceil(8)
    }
}

pub(super) fn valid_depth(color: u8, depth: u8) -> bool {
    match color {
        0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
        2 | 4 | 6 => matches!(depth, 8 | 16),
        3 => matches!(depth, 1 | 2 | 4 | 8),
        _ => false,
    }
}

pub(super) fn unfilter(
    raw: &[u8],
    pos: &mut usize,
    width: u32,
    height: u32,
    header: Header,
) -> EngineResult<Vec<u8>> {
    let stride = header.row_bytes(width);
    let bpp = header.filter_bpp();
    let mut out = vec![0u8; stride * height as usize];
    for y in 0..height as usize {
        if *pos >= raw.len() {
            return Err(unsupported("png scanline truncated".into()));
        }
        let filter = raw[*pos];
        *pos += 1;
        let row = y * stride;
        let src_end = *pos + stride;
        if src_end > raw.len() {
            return Err(unsupported("png scanline truncated".into()));
        }
        out[row..row + stride].copy_from_slice(&raw[*pos..src_end]);
        *pos = src_end;
        let above = row.wrapping_sub(stride);
        for i in 0..stride {
            let a = if i >= bpp {
                out[row + i - bpp] as i32
            } else {
                0
            };
            let b = if y > 0 { out[above + i] as i32 } else { 0 };
            let c = if i >= bpp && y > 0 {
                out[above + i - bpp] as i32
            } else {
                0
            };
            let x = out[row + i] as i32;
            let v = match filter {
                0 => x,
                1 => x + a,
                2 => x + b,
                3 => x + ((a + b) >> 1),
                4 => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((b - c).abs(), (a - c).abs(), (p - c).abs());
                    x + if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                other => return Err(unsupported(format!("unsupported png filter {other}"))),
            };
            out[row + i] = (v & 0xff) as u8;
        }
    }
    Ok(out)
}

fn sample(row: &[u8], index: usize, depth: u8) -> u16 {
    match depth {
        8 => row.get(index).copied().unwrap_or(0) as u16,
        16 => {
            let p = index * 2;
            let hi = row.get(p).copied().unwrap_or(0);
            let lo = row.get(p + 1).copied().unwrap_or(0);
            u16::from(hi) << 8 | u16::from(lo)
        }
        d => {
            let bit = index * d as usize;
            let byte = row.get(bit / 8).copied().unwrap_or(0);
            let shift = 8 - d as usize - (bit % 8);
            let mask = (1u16 << d) - 1;
            ((byte as u16) >> shift) & mask
        }
    }
}

fn scale8(v: u16, depth: u8) -> u8 {
    match depth {
        8 => v as u8,
        16 => (v >> 8) as u8,
        1 => {
            if v == 0 {
                0
            } else {
                255
            }
        }
        2 => (v * 85) as u8,
        4 => (v * 17) as u8,
        _ => v as u8,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_pass(
    header: Header,
    pass: &[u8],
    pass_w: u32,
    pass_h: u32,
    x_start: u32,
    y_start: u32,
    x_step: u32,
    y_step: u32,
    palette: Option<&[u8]>,
    trns: Option<&[u8]>,
    out: &mut [u8],
) -> EngineResult<()> {
    let row_bytes = header.row_bytes(pass_w);
    let depth = header.depth;
    let trns_gray = if header.color == 0 {
        trns.and_then(|t| t.get(0..2))
            .map(|t| u16::from(t[0]) << 8 | u16::from(t[1]))
    } else {
        None
    };
    let trns_rgb = if header.color == 2 {
        trns.and_then(|t| (t.len() >= 6).then_some(t)).map(|t| {
            (
                u16::from(t[0]) << 8 | u16::from(t[1]),
                u16::from(t[2]) << 8 | u16::from(t[3]),
                u16::from(t[4]) << 8 | u16::from(t[5]),
            )
        })
    } else {
        None
    };
    for py in 0..pass_h {
        let row = &pass[py as usize * row_bytes..(py as usize + 1) * row_bytes];
        let y = y_start + py * y_step;
        for px in 0..pass_w {
            let x = x_start + px * x_step;
            let idx = (y as usize * header.width as usize + x as usize) * 4;
            let (r, g, b, a) = match header.color {
                0 => {
                    let v = sample(row, px as usize, depth);
                    let a = if trns_gray == Some(v) { 0 } else { 255 };
                    let c = scale8(v, depth);
                    (c, c, c, a)
                }
                2 => {
                    let base = px as usize * 3;
                    let (r0, g0, b0) = (
                        sample(row, base, depth),
                        sample(row, base + 1, depth),
                        sample(row, base + 2, depth),
                    );
                    let a = if trns_rgb == Some((r0, g0, b0)) {
                        0
                    } else {
                        255
                    };
                    (scale8(r0, depth), scale8(g0, depth), scale8(b0, depth), a)
                }
                3 => {
                    let i = sample(row, px as usize, depth) as usize;
                    let p =
                        palette.ok_or_else(|| unsupported("indexed png missing PLTE".into()))?;
                    if i * 3 + 2 >= p.len() {
                        return Err(unsupported("png palette index out of range".into()));
                    }
                    let a = trns.and_then(|t| t.get(i).copied()).unwrap_or(255);
                    (p[i * 3], p[i * 3 + 1], p[i * 3 + 2], a)
                }
                4 => {
                    let base = px as usize * 2;
                    let v = sample(row, base, depth);
                    let a = sample(row, base + 1, depth);
                    let c = scale8(v, depth);
                    (c, c, c, scale8(a, depth))
                }
                6 => {
                    let base = px as usize * 4;
                    (
                        scale8(sample(row, base, depth), depth),
                        scale8(sample(row, base + 1, depth), depth),
                        scale8(sample(row, base + 2, depth), depth),
                        scale8(sample(row, base + 3, depth), depth),
                    )
                }
                _ => return Err(unsupported("unsupported png colour type".into())),
            };
            out[idx..idx + 4].copy_from_slice(&[r, g, b, a]);
        }
    }
    Ok(())
}

/// Adam7 pass geometry: `(x_start, y_start, x_step, y_step)`.
pub(super) const ADAM7: [(u32, u32, u32, u32); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

pub(super) fn pass_size(full: u32, start: u32, step: u32) -> u32 {
    if full <= start {
        0
    } else {
        (full - start).div_ceil(step)
    }
}

pub(super) fn raw_len(header: Header) -> usize {
    if header.interlace == 0 {
        let stride = header.row_bytes(header.width);
        header.height as usize * (stride + 1)
    } else {
        ADAM7
            .iter()
            .map(|&(xs, ys, xstep, ystep)| {
                let pw = pass_size(header.width, xs, xstep);
                let ph = pass_size(header.height, ys, ystep);
                if pw == 0 || ph == 0 {
                    0
                } else {
                    ph as usize * (header.row_bytes(pw) + 1)
                }
            })
            .sum()
    }
}
