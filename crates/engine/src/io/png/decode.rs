//! `decodePng` — PNG bytes to RGBA pixels, filters 0–4.
//!
//! All five row filters are implemented, `paeth` included, because that is what
//! real files use; the fixtures only exercise them on their own single image,
//! so `tests.rs` re-encodes a gradient under each filter to keep every branch
//! live. The unfilter loop reads the already-restored row above, so it walks by
//! index rather than `split_at_mut`.

use super::crc::{SIG, chunks, crc, read_be32, unsupported};
use crate::constants::{MAX_H_PX, MAX_W_PX};
use crate::errors::{CODE_PNG_SIGNATURE, EngineResult};

/// A decoded image: its dimensions and tightly packed RGBA rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PngImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, row-major, straight (unpremultiplied).
    pub rgba: Vec<u8>,
}

impl PngImage {
    /// Byte offset of canvas pixel `(x, y)`.
    #[inline]
    pub fn offset(&self, x: u32, y: u32) -> usize {
        ((y * self.width + x) * 4) as usize
    }
}

/// `decodePng` — PNG bytes to RGBA pixels, filters 0–4.
pub fn decode_png(bytes: &[u8]) -> EngineResult<PngImage> {
    if bytes.len() < 8 || bytes[..8] != SIG {
        return Err(crate::errors::DrawToolError::new(
            CODE_PNG_SIGNATURE,
            "not a png file",
        ));
    }
    let mut width = 0u32;
    let mut height = 0u32;
    let mut seen_ihdr = false;
    let mut idat: Vec<u8> = Vec::new();

    for (kind, data) in chunks(bytes)? {
        match &kind {
            b"IHDR" => {
                if data.len() != 13 {
                    return Err(unsupported("png IHDR must be 13 bytes".into()));
                }
                seen_ihdr = true;
                width = read_be32(data, 0).expect("13 bytes checked");
                height = read_be32(data, 4).expect("13 bytes checked");
                // depth 8, colour type 6 (RGBA), interlace 0.
                if data[8] != 8 || data[9] != 6 || data[12] != 0 {
                    return Err(unsupported(
                        "only 8-bit RGBA non-interlaced png supported".into(),
                    ));
                }
                if width == 0
                    || height == 0
                    || u64::from(width) * u64::from(height)
                        > u64::from(MAX_W_PX) * u64::from(MAX_H_PX)
                {
                    return Err(unsupported(format!(
                        "unsupported png size {width}x{height}"
                    )));
                }
            }
            b"IDAT" => idat.extend_from_slice(data),
            _ => {}
        }
    }
    if !seen_ihdr {
        return Err(unsupported("png missing IHDR".into()));
    }

    let mut raw = Vec::new();
    std::io::Read::read_to_end(&mut flate2::read::ZlibDecoder::new(&idat[..]), &mut raw)
        .map_err(|_| crc("png idat decompress failed"))?;

    let stride = (width * 4) as usize;
    if raw.len() != height as usize * (stride + 1) {
        return Err(unsupported("png scanline size mismatch".into()));
    }

    let mut out = vec![0u8; height as usize * stride];
    for y in 0..height as usize {
        let filter = raw[y * (stride + 1)];
        let row = y * stride;
        out[row..row + stride].copy_from_slice(&raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)]);
        let above = row.wrapping_sub(stride);
        for i in 0..stride {
            let a = if i >= 4 { out[row + i - 4] as i32 } else { 0 };
            let b = if y > 0 { out[above + i] as i32 } else { 0 };
            let c = if i >= 4 && y > 0 {
                out[above + i - 4] as i32
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
                    let (pa, pb, pc) = ((b - c).abs(), (a - c).abs(), p.abs());
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
    Ok(PngImage {
        width,
        height,
        rgba: out,
    })
}
