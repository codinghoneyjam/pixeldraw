//! Base64 text encoding — ported from `src/io/base64.js`.
//!
//! The JS tree had two code paths here: `btoa`/`atob` in the browser and
//! `Buffer` under Node. Both are gone in the native build, so this module picks
//! one implementation. The exported surface stays the same shape — two plain
//! functions on byte slices and strings — because the serializers and the PNG
//! chunk codec both speak it.

use crate::constants::SCHEMA_VERSION;
use crate::errors::{CODE_SCHEMA, DrawToolError, EngineResult};

use base64::Engine as _;
use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};

/// The standard `A-Za-z0-9+/` alphabet with `=` padding, the set the JS
/// charset guard `/^[A-Za-z0-9+/]*={0,2}$/` allowed.
const STANDARD: GeneralPurposeConfig = GeneralPurposeConfig::new()
    .with_encode_padding(true)
    .with_decode_padding_mode(base64::engine::DecodePaddingMode::RequireCanonical);

fn engine() -> GeneralPurpose {
    GeneralPurpose::new(&alphabet::STANDARD, STANDARD)
}

/// `bytesToBase64` — encode, always with canonical `=` padding.
///
/// The JS chunked its input to dodge a call-stack overflow; that limit belongs
/// to `String.fromCharCode.apply`, so it disappears with the rewrite.
pub fn bytes_to_base64(bytes: &[u8]) -> String {
    engine().encode(bytes)
}

/// `base64ToBytes` — decode. A malformed string raises `SCHEMA`.
///
/// The rejection rule is the JS regex's: only the standard alphabet plus up to
/// two trailing `=`. Whitespace, URL-safe `-`/`_` and missing padding are all
/// errors, which is what keeps a hand-edited document file from silently
/// decoding to garbage.
pub fn base64_to_bytes(s: &str) -> EngineResult<Vec<u8>> {
    if !is_base64_shaped(s) {
        return Err(invalid_base64());
    }
    engine().decode(s).map_err(|_| invalid_base64())
}

/// The charset half of the JS guard.
fn is_base64_shaped(s: &str) -> bool {
    let body = s.trim_end_matches('=');
    if s.len() - body.len() > 2 {
        return false;
    }
    body.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

fn invalid_base64() -> DrawToolError {
    DrawToolError::new(CODE_SCHEMA, "invalid base64 string")
}

/// Keeps `SCHEMA_VERSION` in the dependency graph of the docs, and lets a caller
/// assert the codec it is about to write matches the schema it claims.
pub const fn codec_schema_version() -> &'static str {
    SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    // <META - ROLE : parity with tests/io_basic.test.mjs base64 cases
    #[test]
    fn round_trip() {
        for len in [0usize, 1, 2, 3, 4, 5, 255, 1024] {
            let bytes: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            let text = bytes_to_base64(&bytes);
            assert_eq!(base64_to_bytes(&text).expect("valid"), bytes, "len {len}");
        }
    }

    #[test]
    fn known_vectors() {
        // RFC 4648 test vectors.
        assert_eq!(bytes_to_base64(b""), "");
        assert_eq!(bytes_to_base64(b"f"), "Zg==");
        assert_eq!(bytes_to_base64(b"fo"), "Zm8=");
        assert_eq!(bytes_to_base64(b"foo"), "Zm9v");
        assert_eq!(bytes_to_base64(b"foob"), "Zm9vYg==");
        assert_eq!(bytes_to_base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(bytes_to_base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_to_bytes("Zm9vYmFy").expect("valid"), b"foobar");
    }

    #[test]
    fn padding_is_canonical() {
        assert_eq!(&bytes_to_base64(&[0xff])[2..], "==");
        assert_eq!(&bytes_to_base64(&[0xff, 0xff])[3..], "=");
        // A missing pad is a decode error, exactly like the JS charset guard.
        assert!(base64_to_bytes("Zg").is_err());
        assert!(base64_to_bytes("Zg=").is_err());
    }

    #[test]
    fn bad_characters_raise_schema() {
        for bad in ["Zm9v YmFy", "Zm9v-YmFy", "Zm9v_YmFy", "=====", "===="] {
            match base64_to_bytes(bad) {
                Err(e) => assert_eq!(e.code, CODE_SCHEMA, "{bad}"),
                Ok(_) => panic!("{bad:?} must be rejected"),
            }
        }
    }

    #[test]
    fn an_empty_string_decodes_to_nothing() {
        assert_eq!(base64_to_bytes("").expect("valid"), Vec::<u8>::new());
    }
}
