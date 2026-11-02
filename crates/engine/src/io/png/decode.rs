//! `decodePng` ??PNG bytes to tightly packed RGBA rows.
//!
//! The importer accepts the full PNG colour/depth space (grayscale, RGB,
//! indexed, gray+alpha, RGBA at 1/2/4/8/16-bit, with or without Adam7
//! interlacing) so the existing `asset_work` PNGs and reference images open.
//! Filters 0?? are the PNG set. Ancillary chunks are retained in
//! [`PngImage::metadata`] so Pixeldraw's atlas payload survives a save/reopen.

mod scanlines;
use scanlines::{ADAM7, Header, pass_size, raw_len, unfilter, valid_depth, write_pass};

use super::crc::{SIG, chunks, crc, read_be32, unsupported};
use crate::constants::{MAX_H_PX, MAX_W_PX};
use crate::errors::{CODE_PNG_SIGNATURE, DrawToolError, EngineResult};
use std::io::Read;

/// A decoded image: its dimensions, tightly packed RGBA rows, and ancillary
/// chunks (kind + raw payload) in file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PngImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, row-major, straight (unpremultiplied).
    pub rgba: Vec<u8>,
    pub metadata: Vec<(String, Vec<u8>)>,
}

impl PngImage {
    /// Byte offset of canvas pixel `(x, y)`.
    #[inline]
    pub fn offset(&self, x: u32, y: u32) -> usize {
        ((y * self.width + x) * 4) as usize
    }

    /// First ancillary chunk with `kind`, for Pixeldraw metadata.
    pub fn metadata_chunk(&self, kind: &str) -> Option<&[u8]> {
        self.metadata
            .iter()
            .find(|(k, _)| k == kind)
            .map(|(_, d)| d.as_slice())
    }
}

/// `decodePng` ??PNG bytes to RGBA pixels.
pub fn decode_png(bytes: &[u8]) -> EngineResult<PngImage> {
    decode_png_impl(bytes, u64::from(MAX_W_PX) * u64::from(MAX_H_PX))
}

/// Decode a PNG against the atlas pixel budget instead of the editor's caps.
pub fn decode_png_large(bytes: &[u8]) -> EngineResult<PngImage> {
    decode_png_impl(bytes, crate::atlas::ATLAS_MAX_PX)
}

fn decode_png_impl(bytes: &[u8], max_px: u64) -> EngineResult<PngImage> {
    if bytes.len() < 8 || bytes[..8] != SIG {
        return Err(DrawToolError::new(CODE_PNG_SIGNATURE, "not a png file"));
    }
    let mut header: Option<Header> = None;
    let mut idat: Vec<u8> = Vec::new();
    let mut palette: Option<Vec<u8>> = None;
    let mut trns: Option<Vec<u8>> = None;
    let mut metadata: Vec<(String, Vec<u8>)> = Vec::new();

    for (kind, data) in chunks(bytes)? {
        match &kind {
            b"IHDR" => {
                if data.len() != 13 {
                    return Err(unsupported("png IHDR must be 13 bytes".into()));
                }
                if header.is_some() {
                    return Err(unsupported("duplicate png IHDR".into()));
                }
                let width = read_be32(data, 0).expect("13 bytes checked");
                let height = read_be32(data, 4).expect("13 bytes checked");
                let depth = data[8];
                let color = data[9];
                let interlace = data[12];
                if data[10] != 0 || data[11] != 0 {
                    return Err(unsupported(
                        "unsupported png compression/filter method".into(),
                    ));
                }
                if !valid_depth(color, depth) || interlace > 1 {
                    return Err(unsupported(format!(
                        "unsupported png depth/colour/interlace {depth}/{color}/{interlace}"
                    )));
                }
                if width == 0 || height == 0 || u64::from(width) * u64::from(height) > max_px {
                    return Err(unsupported(format!(
                        "unsupported png size {width}x{height}"
                    )));
                }
                header = Some(Header {
                    width,
                    height,
                    depth,
                    color,
                    interlace,
                });
            }
            b"PLTE" => {
                if data.len() % 3 != 0 || data.len() > 768 {
                    return Err(unsupported("invalid png palette".into()));
                }
                palette = Some(data.to_vec());
            }
            b"tRNS" => trns = Some(data.to_vec()),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {
                // Keep ancillary chunks (lowercase first letter). Our `dtAt`
                // atlas payload is one of them; text/exif chunks are harmless.
                if kind[0].is_ascii_lowercase() {
                    let name = String::from_utf8_lossy(&kind).into_owned();
                    metadata.push((name, data.to_vec()));
                }
            }
        }
    }

    let header = header.ok_or_else(|| unsupported("png missing IHDR".into()))?;
    if header.color == 3 && palette.is_none() {
        return Err(unsupported("indexed png missing PLTE".into()));
    }
    if idat.is_empty() {
        return Err(unsupported("png missing IDAT".into()));
    }

    let expected = raw_len(header);
    let mut raw = Vec::with_capacity(expected);
    let decoder = flate2::read::ZlibDecoder::new(&idat[..]);
    decoder
        .take(expected as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|_| crc("png idat decompress failed"))?;
    if raw.len() != expected {
        return Err(unsupported("png scanline size mismatch".into()));
    }

    let stride = (header.width * 4) as usize;
    let mut out = vec![0u8; stride * header.height as usize];
    let mut pos = 0usize;
    if header.interlace == 0 {
        let pass = unfilter(&raw, &mut pos, header.width, header.height, header)?;
        write_pass(
            header,
            &pass,
            header.width,
            header.height,
            0,
            0,
            1,
            1,
            palette.as_deref(),
            trns.as_deref(),
            &mut out,
        )?;
    } else {
        for &(xs, ys, xstep, ystep) in &ADAM7 {
            let pw = pass_size(header.width, xs, xstep);
            let ph = pass_size(header.height, ys, ystep);
            if pw == 0 || ph == 0 {
                continue;
            }
            let pass = unfilter(&raw, &mut pos, pw, ph, header)?;
            write_pass(
                header,
                &pass,
                pw,
                ph,
                xs,
                ys,
                xstep,
                ystep,
                palette.as_deref(),
                trns.as_deref(),
                &mut out,
            )?;
        }
    }
    if pos != raw.len() {
        return Err(unsupported("png scanline size mismatch".into()));
    }

    Ok(PngImage {
        width: header.width,
        height: header.height,
        rgba: out,
        metadata,
    })
}
