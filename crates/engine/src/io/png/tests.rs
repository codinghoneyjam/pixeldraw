use super::crc::{SIG, be32, crc32, push_chunk, read_be32};
use super::*;
use crate::constants::{CHUNK_LEN, CHUNK_PX};
use crate::errors::{CODE_PNG_CRC, CODE_PNG_SIGNATURE, CODE_PNG_UNSUPPORTED};
use crate::io::base64::{base64_to_bytes, bytes_to_base64};

/// `tests/fixtures/png_cases.json`, loaded once.
fn cases() -> &'static Vec<serde_json::Value> {
    static V: std::sync::OnceLock<Vec<serde_json::Value>> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        let text = crate::fixtures::web_file("tests/fixtures/png_cases.json");
        serde_json::from_str::<serde_json::Value>(&text).expect("png_cases.json parses")["cases"]
            .as_array()
            .expect("cases")
            .clone()
    })
}

// <META - ROLE : golden png_cases group, all 5 cases, same as tests/png_cases.test.mjs
mod cases_01;
mod cases_02;
