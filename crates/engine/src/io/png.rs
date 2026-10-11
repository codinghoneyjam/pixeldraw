//! PNG codec — ported from `src/io/png.js`.
//!
//! Scope is deliberately the same as the JS one: **8-bit RGBA, non-interlaced**
//! on read; filter-0 rows and a single IDAT on write. Nothing else is accepted,
//! and the failure is a coded [`DrawToolError`], never a panic.
//!
//! ## What changed shape
//!
//! * `encodePng`/`decodePng` were `async` only because they went through
//!   `CompressionStream`. With `flate2` they are plain functions.
//! * The compressed bytes are **not** byte-identical to the JS output — zlib's
//!   deflate and miniz_oxide's differ. The plan accepts this (§2난제 4): the
//!   gate is *decoded pixel* equality, plus a save→reload round trip.
//!
//! ## Filters
//!
//! Decode implements all five PNG row filters (0–4) including the `paeth`
//! predictor, because that is what real files use. Encode writes filter 0, the
//! simplest, matching the JS encoder.

use crate::constants::{MAX_H_PX, MAX_W_PX};
use crate::errors::{
    CODE_PNG_CRC, CODE_PNG_SIGNATURE, CODE_PNG_UNSUPPORTED, DrawToolError, EngineResult,
};

/// The 8-byte PNG signature.
const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

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

/// CRC-32 over a byte slice, the PNG polynomial `0xedb88320`.
///
/// Hand-rolled rather than pulled from `crc32fast` (which `flate2` already
/// brings) because the table is 8 lines and the whole point of the engine's
/// small-dependency rule is not to widen the graph for it.
///
/// The 256-entry table is built **once** and shared, exactly as the JS module
/// built it at import time. Recomputing it per call burns 2048 iterations for
/// nothing, which on a chunk stream is most of the work.
fn crc32(bytes: &[u8]) -> u32 {
    let table = crc_table();
    let mut c = 0xffff_ffff;
    for &b in bytes {
        c = table[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xffff_ffff
}

/// The lazily built CRC table, behind the same `OnceLock` the brush footprint
/// cache uses — the workspace forbids `unsafe`, so no `static mut` trick.
fn crc_table() -> &'static [u32; 256] {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (n, slot) in table.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *slot = c;
        }
        table
    })
}

/// Big-endian `u32`, the width the PNG chunk headers use.
#[inline]
fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

/// Read a big-endian `u32` at `p`, or `None` past the end.
fn read_be32(b: &[u8], p: usize) -> Option<u32> {
    let raw = b.get(p..p + 4)?;
    Some(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

/// Collect `(kind, data)` pairs, validating structure and CRC as it walks.
///
/// Every structural failure is `PNG_CRC`, matching the JS decoder's messages —
/// that code is overloaded for "truncated" as well as "bad checksum" there, and
/// the catalog has no separate one.
fn chunks(bytes: &[u8]) -> EngineResult<Vec<([u8; 4], &[u8])>> {
    let mut out = Vec::new();
    let mut p = 8usize;
    while p < bytes.len() {
        let Some(len) = read_be32(bytes, p) else {
            return Err(crc("truncated png chunk header"));
        };
        if p + 12 + len as usize > bytes.len() {
            return Err(crc("truncated png chunk data"));
        }
        let body_start = p + 4;
        let data_start = body_start + 4;
        let data_end = data_start + len as usize;
        let declared = read_be32(bytes, data_end).expect("length checked above");
        if crc32(&bytes[body_start..data_end]) != declared {
            return Err(crc("png chunk crc mismatch"));
        }
        let mut kind = [0u8; 4];
        kind.copy_from_slice(&bytes[body_start..data_start]);
        let is_end = &kind == b"IEND";
        out.push((kind, &bytes[data_start..data_end]));
        if is_end {
            break;
        }
        p = data_end + 4;
    }
    Ok(out)
}

fn crc(message: &str) -> DrawToolError {
    DrawToolError::new(CODE_PNG_CRC, message)
}

fn unsupported(message: String) -> DrawToolError {
    DrawToolError::new(CODE_PNG_UNSUPPORTED, message)
}

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

    // Filter byte 0 in front of every row, no other filtering.
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

/// `makeChunk` — length, type, data, CRC.
fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&be32(data.len() as u32));
    let body_start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[body_start..]);
    out.extend_from_slice(&be32(crc));
}

/// `decodePng` — PNG bytes to RGBA pixels, filters 0–4.
pub fn decode_png(bytes: &[u8]) -> EngineResult<PngImage> {
    if bytes.len() < 8 || bytes[..8] != SIG {
        return Err(DrawToolError::new(CODE_PNG_SIGNATURE, "not a png file"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::base64::base64_to_bytes;
    use crate::io::base64::bytes_to_base64;

    /// `tests/fixtures/png_cases.json`, loaded once.
    fn cases() -> &'static Vec<serde_json::Value> {
        static V: std::sync::OnceLock<Vec<serde_json::Value>> = std::sync::OnceLock::new();
        V.get_or_init(|| {
            let text = crate::fixtures::repo_file("tests/fixtures/png_cases.json");
            serde_json::from_str::<serde_json::Value>(&text)
                .expect("png_cases.json parses")["cases"]
                .as_array()
                .expect("cases")
                .clone()
        })
    }

    // <META - ROLE : golden png_cases group, all 5 cases, same as tests/png_cases.test.mjs
    #[test]
    fn golden_png_cases() {
        let mut decoded = 0;
        for case in cases() {
            let name = case["name"].as_str().unwrap();
            let png = base64_to_bytes(case["png"].as_str().unwrap()).expect("base64 png");
            let want_error = case["expect_error"].as_str();
            match decode_png(&png) {
                Ok(img) => {
                    assert!(
                        want_error.is_none(),
                        "{name}: decoded but expected {want_error:?}"
                    );
                    assert_eq!(img.width, case["width"].as_u64().unwrap() as u32, "{name}");
                    assert_eq!(
                        img.height,
                        case["height"].as_u64().unwrap() as u32,
                        "{name}"
                    );
                    let hex = case["rgba_hex"].as_str().unwrap();
                    let want = hex_bytes(hex);
                    assert_eq!(img.rgba, want, "{name}");
                    decoded += 1;
                }
                Err(e) => {
                    let code = want_error.unwrap_or_else(|| {
                        panic!(
                            "{name}: decode failed with {} but no error was expected",
                            e.code
                        )
                    });
                    assert_eq!(e.code, code, "{name}");
                }
            }
        }
        assert_eq!(decoded, 1, "one case decodes, four are error vectors");
        assert_eq!(cases().len(), 5, "the png_cases group has 5 cases");
    }

    /// `"0A141EFF"` → `[0x0a, 0x14, 0x1e, 0xff]`.
    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
            .collect()
    }

    #[test]
    fn round_trip_through_our_own_encoder() {
        let w = 7u32;
        let h = 5u32;
        let rgba: Vec<u8> = (0..(w * h * 4)).map(|i| (i * 7 % 256) as u8).collect();
        let bytes = encode_png(&rgba, w, h).expect("encode");
        let img = decode_png(&bytes).expect("decode");
        assert_eq!((img.width, img.height), (w, h));
        assert_eq!(img.rgba, rgba);
    }

    #[test]
    fn the_encoder_emits_canonical_chunk_order() {
        let rgba = vec![0u8; 4 * 4 * 4];
        let bytes = encode_png(&rgba, 4, 4).expect("encode");
        assert_eq!(&bytes[..8], &SIG);
        // IHDR, IDAT, IEND — in that order, with correct lengths and CRCs.
        let mut p = 8;
        let mut kinds = Vec::new();
        while p < bytes.len() {
            let len = read_be32(&bytes, p).expect("length");
            let kind = [bytes[p + 4], bytes[p + 5], bytes[p + 6], bytes[p + 7]];
            let body = &bytes[p + 4..p + 8 + len as usize];
            assert_eq!(
                crc32(body),
                read_be32(&bytes, p + 8 + len as usize).unwrap(),
                "crc"
            );
            kinds.push(String::from_utf8_lossy(&kind).to_string());
            p += 12 + len as usize;
        }
        assert_eq!(kinds, vec!["IHDR", "IDAT", "IEND"]);
    }

    #[test]
    fn a_flipped_crc_byte_is_rejected() {
        let rgba = vec![0x11u8; 4 * 4 * 4];
        let mut bytes = encode_png(&rgba, 4, 4).expect("encode");
        let last = bytes.len() - 5; // inside the IEND CRC
        bytes[last] ^= 0xff;
        match decode_png(&bytes) {
            Err(e) => assert_eq!(e.code, CODE_PNG_CRC),
            Ok(_) => panic!("a corrupted CRC must be rejected"),
        }
    }

    #[test]
    fn a_truncated_file_is_a_coded_error_not_a_panic() {
        let rgba = vec![0x22u8; 4 * 4 * 4];
        let bytes = encode_png(&rgba, 4, 4).expect("encode");
        // Anything under the 8-byte signature fails there; a file that is only
        // the signature has no IHDR; a mangled chunk stream fails in the walker.
        // All three are coded errors, none is a panic.
        for cut in [0usize, 4, 7] {
            match decode_png(&bytes[..cut]) {
                Err(e) => assert_eq!(e.code, CODE_PNG_SIGNATURE, "cut at {cut}"),
                Ok(_) => panic!("a {cut}-byte file must be rejected"),
            }
        }
        for cut in [8usize, 9, 20, 33, bytes.len() - 3, bytes.len() - 1] {
            match decode_png(&bytes[..cut]) {
                Err(e) => {
                    let code = e.code;
                    assert!(
                        code == CODE_PNG_CRC || code == CODE_PNG_UNSUPPORTED,
                        "cut at {cut} gave {code}"
                    );
                }
                Ok(_) => panic!("a truncated file must be rejected"),
            }
        }
        let whole = decode_png(&bytes).expect("the untruncated file decodes");
        assert_eq!(whole.rgba, rgba);
    }

    #[test]
    fn a_non_png_file_is_a_signature_error() {
        match decode_png(&bytes_to_base64(b"definitely not a png at all").into_bytes()) {
            Err(e) => assert_eq!(e.code, CODE_PNG_SIGNATURE),
            Ok(_) => panic!("garbage must be rejected"),
        }
    }

    #[test]
    fn zero_sized_images_are_unsupported() {
        assert!(encode_png(&[], 0, 4).is_err());
        assert!(encode_png(&[], 4, 0).is_err());
        // A buffer that does not match the dimensions is caught before writing.
        assert!(encode_png(&[0u8; 3], 4, 4).is_err());
    }

    #[test]
    fn each_row_filter_reverses_exactly() {
        // A gradient image re-encoded with each of the five filters must decode
        // to the same pixels — this is the branch coverage the fixtures only
        // exercise on their own single image.
        let w = 9usize;
        let h = 6usize;
        let src: Vec<u8> = (0..w * h * 4).map(|i| ((i * 13 + 7) % 256) as u8).collect();
        for filter in 0u8..5 {
            let mut raw = Vec::new();
            for y in 0..h {
                raw.push(filter);
                let row = &src[y * w * 4..(y + 1) * w * 4];
                for (i, &v) in row.iter().enumerate() {
                    let a = if i >= 4 { row[i - 4] as i32 } else { 0 };
                    let b = if y > 0 {
                        let prev = &src[(y - 1) * w * 4..y * w * 4];
                        prev[i] as i32
                    } else {
                        0
                    };
                    let c = if i >= 4 && y > 0 {
                        let prev = &src[(y - 1) * w * 4..y * w * 4];
                        prev[i - 4] as i32
                    } else {
                        0
                    };
                    let x = v as i32;
                    let f = match filter {
                        0 => x,
                        1 => x - a,
                        2 => x - b,
                        3 => x - ((a + b) >> 1),
                        _ => {
                            let p = a + b - c;
                            let (pa, pb, pc) = ((b - c).abs(), (a - c).abs(), p.abs());
                            x - if pa <= pb && pa <= pc {
                                a
                            } else if pb <= pc {
                                b
                            } else {
                                c
                            }
                        }
                    };
                    raw.push((f & 0xff) as u8);
                }
            }
            let mut idat = Vec::new();
            let mut enc =
                flate2::write::ZlibEncoder::new(&mut idat, flate2::Compression::default());
            std::io::Write::write_all(&mut enc, &raw).expect("deflate");
            enc.finish().expect("deflate");
            let mut ihdr = Vec::new();
            ihdr.extend_from_slice(&be32(w as u32));
            ihdr.extend_from_slice(&be32(h as u32));
            ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&SIG);
            push_chunk(&mut bytes, b"IHDR", &ihdr);
            push_chunk(&mut bytes, b"IDAT", &idat);
            push_chunk(&mut bytes, b"IEND", &[]);
            let img = decode_png(&bytes).expect("decode");
            assert_eq!(img.rgba, src, "filter {filter}");
        }
    }
}
