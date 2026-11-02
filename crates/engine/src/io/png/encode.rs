//! `encodePng` — RGBA pixels to PNG bytes.
//!
//! Filter 0 in front of every row and a single IDAT, which is exactly the JS
//! encoder's scope. Ancillary chunks (Pixeldraw's atlas metadata) can be
//! inserted before the IDAT with [`encode_png_with_chunks`].

use std::cell::RefCell;

use flate2::{Compress, Compression, FlushCompress, Status};

use super::crc::{SIG, be32, push_chunk, unsupported};
use crate::constants::{CHUNK_LEN, CHUNK_PX, MAX_H_PX, MAX_W_PX};
use crate::errors::EngineResult;

/// `encodePng` — RGBA pixels to PNG bytes (filter 0, one IDAT).
pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> EngineResult<Vec<u8>> {
    encode_png_with_chunks(rgba, width, height, &[])
}

/// Same image, with ancillary `(kind, payload)` chunks before the IDAT.
pub fn encode_png_with_chunks(
    rgba: &[u8],
    width: u32,
    height: u32,
    chunks: &[(&str, &[u8])],
) -> EngineResult<Vec<u8>> {
    encode_png_impl(rgba, width, height, chunks, MAX_W_PX, MAX_H_PX, u64::MAX)
}

/// Atlas-sized PNG: only the atlas pixel budget applies, not the editor's
/// per-axis document caps.
pub fn encode_png_large(rgba: &[u8], width: u32, height: u32) -> EngineResult<Vec<u8>> {
    encode_png_large_with_chunks(rgba, width, height, &[])
}

/// [`encode_png_large`] with ancillary chunks (`dtAt` metadata).
pub fn encode_png_large_with_chunks(
    rgba: &[u8],
    width: u32,
    height: u32,
    chunks: &[(&str, &[u8])],
) -> EngineResult<Vec<u8>> {
    encode_png_impl(
        rgba,
        width,
        height,
        chunks,
        u32::MAX,
        u32::MAX,
        crate::atlas::ATLAS_MAX_PX,
    )
}

fn encode_png_impl(
    rgba: &[u8],
    width: u32,
    height: u32,
    chunks: &[(&str, &[u8])],
    max_w: u32,
    max_h: u32,
    max_px: u64,
) -> EngineResult<Vec<u8>> {
    if width == 0
        || height == 0
        || width > max_w
        || height > max_h
        || u64::from(width) * u64::from(height) > max_px
    {
        return Err(unsupported(format!("bad png size {width}x{height}")));
    }
    let stride = (width * 4) as usize;
    if rgba.len() != stride * height as usize {
        return Err(unsupported(format!(
            "bad png buffer {} bytes for {width}x{height}",
            rgba.len()
        )));
    }
    let mut kinds = Vec::with_capacity(chunks.len());
    for (kind, _) in chunks {
        let bytes = kind.as_bytes();
        if bytes.len() != 4 || !bytes.iter().all(u8::is_ascii_alphabetic) {
            return Err(unsupported(format!("bad png chunk kind {kind:?}")));
        }
        let mut k = [0u8; 4];
        k.copy_from_slice(bytes);
        kinds.push(k);
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
    for ((_, data), kind) in chunks.iter().zip(kinds.iter()) {
        push_chunk(&mut out, kind, data);
    }
    push_chunk(&mut out, b"IDAT", &idat);
    push_chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

/// Per-thread deflate state for the save path's 32×32 chunk PNGs.
///
/// `document_to_json` calls this once per non-empty chunk (37,504 times on the
/// busy benchmark document). A fresh `ZlibEncoder` per chunk spends almost all
/// of its time in miniz state allocation and initialisation rather than on the
/// 4 KB payload; resetting one compressor per thread removes that cost while
/// keeping the encoded bytes identical at the default level.
struct ChunkScratch {
    raw: Vec<u8>,
    idat: Vec<u8>,
    deflate: Compress,
}

thread_local! {
    static CHUNK_SCRATCH: RefCell<ChunkScratch> = RefCell::new(ChunkScratch {
        raw: Vec::with_capacity(CHUNK_PX as usize * ((CHUNK_PX * 4 + 1) as usize)),
        idat: Vec::with_capacity(64 * 1024),
        deflate: Compress::new(Compression::default(), true),
    });
}

/// Encode one `CHUNK_PX`×`CHUNK_PX` RGBA chunk for `document_to_json`.
///
/// Same filter-0 rows and zlib settings as [`encode_png`], but the raw row
/// buffer and the compressor are reused per rayon worker. The returned bytes
/// are still a complete, ordinary PNG; only the per-chunk allocation is gone.
pub(crate) fn encode_png_chunk(rgba: &[u8]) -> EngineResult<Vec<u8>> {
    if rgba.len() != CHUNK_LEN {
        return Err(unsupported(format!(
            "bad chunk buffer {} bytes for {CHUNK_PX}x{CHUNK_PX}",
            rgba.len()
        )));
    }
    CHUNK_SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        let scratch = &mut *scratch;
        scratch.raw.clear();
        let stride = (CHUNK_PX * 4) as usize;
        for y in 0..CHUNK_PX as usize {
            scratch.raw.push(0);
            scratch
                .raw
                .extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
        }
        scratch.deflate.reset();
        scratch.idat.clear();
        let status = scratch
            .deflate
            .compress_vec(&scratch.raw, &mut scratch.idat, FlushCompress::Finish)
            .map_err(|e| unsupported(format!("deflate: {e}")))?;
        if status != Status::StreamEnd {
            return Err(unsupported(format!(
                "deflate did not finish chunk png: {status:?}"
            )));
        }

        let ihdr = {
            let mut v = Vec::with_capacity(13);
            v.extend_from_slice(&be32(CHUNK_PX));
            v.extend_from_slice(&be32(CHUNK_PX));
            v.extend_from_slice(&[8, 6, 0, 0, 0]);
            v
        };
        let mut out = Vec::with_capacity(SIG.len() + 3 * 12 + ihdr.len() + scratch.idat.len());
        out.extend_from_slice(&SIG);
        push_chunk(&mut out, b"IHDR", &ihdr);
        push_chunk(&mut out, b"IDAT", &scratch.idat);
        push_chunk(&mut out, b"IEND", &[]);
        Ok(out)
    })
}
