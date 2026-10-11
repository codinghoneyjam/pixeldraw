use super::crc::{SIG, be32, crc32, push_chunk, read_be32};
use super::*;
use crate::errors::{CODE_PNG_CRC, CODE_PNG_SIGNATURE, CODE_PNG_UNSUPPORTED};
use crate::io::base64::{base64_to_bytes, bytes_to_base64};

/// `tests/fixtures/png_cases.json`, loaded once.
fn cases() -> &'static Vec<serde_json::Value> {
    static V: std::sync::OnceLock<Vec<serde_json::Value>> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        let text = crate::fixtures::repo_file("tests/fixtures/png_cases.json");
        serde_json::from_str::<serde_json::Value>(&text).expect("png_cases.json parses")["cases"]
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
                        let (pa, pb, pc) = ((b - c).abs(), (a - c).abs(), (p - c).abs());
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
        let mut enc = flate2::write::ZlibEncoder::new(&mut idat, flate2::Compression::default());
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
