//! `encodePng` — RGBA pixels to PNG bytes.
//!
//! Filter 0 in front of every row and a single IDAT, which is exactly the JS
//! encoder's scope. The compressed bytes are **not** byte-identical to the JS
//! output — zlib's deflate and miniz_oxide's differ; the plan accepts this and
//! the gate is decoded-pixel equality plus a save→reload round trip.

use super::crc::{SIG, be32, push_chunk, unsupported};
use crate::constants::{MAX_H_PX, MAX_W_PX};
use crate::errors::EngineResult;

/// `encodePng` — RGBA pixels to PNG bytes (filter 0, one IDAT).
pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> EngineResult<Vec<u8>> {
    if width == 0 || height == 0 || width > MAX_W_PX || height > MAX_H_PX {
        return Err(unsupported(format!("bad png size {width}x{height}")));
    }
    let stride = (width * 4) as usize;
    if rgba.len() != stride * height as usize {
        return Err(unsupported(format!(
            "bad png buffer {} bytes for {width}x{height}",
            rgba.len()
        )));
    }

    let mut raw = Vec::with_capacity(height as usize * (stride + 1));
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
    }

    let mut idat = Vec::new();
    let mut enc = flate2::write::ZlibEncoder::new(&mut idat, flate2::Compression::default());
    std::io::Write::write_all(&mut enc, &raw).map_err(|e| unsupported(format!("deflate: {e}")))?;
    enc.finish()
        .map_err(|e| unsupported(format!("deflate: {e}")))?;

    let ihdr = {
        let mut v = Vec::with_capacity(13);
        v.extend_from_slice(&be32(width));
        v.extend_from_slice(&be32(height));
        v.extend_from_slice(&[8, 6, 0, 0, 0]); // depth, colour type, compression, filter, interlace
        v
    };

    let mut out = Vec::with_capacity(SIG.len() + 3 * 12 + ihdr.len() + idat.len());
    out.extend_from_slice(&SIG);
    push_chunk(&mut out, b"IHDR", &ihdr);
    push_chunk(&mut out, b"IDAT", &idat);
    push_chunk(&mut out, b"IEND", &[]);
    Ok(out)
}
