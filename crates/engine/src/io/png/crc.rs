//! The byte-level container layer both codecs share: the signature, the
//! big-endian chunk words, the CRC-32 table and the chunk walker.
//!
//! Every structural failure is `PNG_CRC`, matching the JS decoder's messages —
//! that code is overloaded for "truncated" as well as "bad checksum" there, and
//! the catalog has no separate one.

use crate::errors::{CODE_PNG_CRC, CODE_PNG_UNSUPPORTED, DrawToolError, EngineResult};

/// The 8-byte PNG signature.
pub(super) const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// Big-endian `u32`, the width the PNG chunk headers use.
#[inline]
pub(super) fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

/// Read a big-endian `u32` at `p`, or `None` past the end.
pub(super) fn read_be32(b: &[u8], p: usize) -> Option<u32> {
    let raw = b.get(p..p + 4)?;
    Some(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

/// CRC-32 over a byte slice, the PNG polynomial `0xedb88320`.
///
/// Hand-rolled rather than pulled from `crc32fast` (which `flate2` already
/// brings) because the table is 8 lines and the engine's small-dependency rule
/// is not worth widening the graph for. The 256-entry table is built **once**
/// and shared, exactly as the JS module built it at import time; recomputing it
/// per call burns 2048 iterations for nothing.
pub(super) fn crc32(bytes: &[u8]) -> u32 {
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

pub(super) fn crc(message: &str) -> DrawToolError {
    DrawToolError::new(CODE_PNG_CRC, message)
}

pub(super) fn unsupported(message: String) -> DrawToolError {
    DrawToolError::new(CODE_PNG_UNSUPPORTED, message)
}

/// Collect `(kind, data)` pairs, validating structure and CRC as it walks.
pub(super) fn chunks(bytes: &[u8]) -> EngineResult<Vec<([u8; 4], &[u8])>> {
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

/// `makeChunk` — length, type, data, CRC.
pub(super) fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&be32(data.len() as u32));
    let body_start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[body_start..]);
    out.extend_from_slice(&be32(crc));
}
