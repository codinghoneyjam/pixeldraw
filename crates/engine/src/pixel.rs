//! Pixel packing + hex parsing (docs/core.md, pixel.js).
//!
//! Ported 1:1 from `src/core/pixel.js`. A pixel is a packed `u32`:
//! `a << 24 | b << 16 | g << 8 | r` on a little-endian host, which is what the
//! JS `Uint32Array` view over a `Uint8ClampedArray` produced. Alpha 0 always
//! packs to 0 — the transparency normalisation of docs/contract.md §2.

/// Transparent packed value; `p == TRANSPARENT` is the emptiness test.
pub const TRANSPARENT: u32 = 0;

/// A straight (unpremultiplied) RGBA quadruple, as the JS arrays were.
pub type Rgba = [u8; 4];

/// JS `Math.round(Number(v))` then clamp to 0..255.
///
/// `Math.round` rounds halves toward +∞ and `f64::round` rounds them away from
/// zero, but every negative result is clamped to 0 by both, so the exported
/// byte is identical.
#[inline]
fn clamp_byte(v: f64) -> u8 {
    let rounded = v.round();
    if !rounded.is_finite() {
        return 0;
    }
    if rounded < 0.0 {
        0
    } else if rounded > 255.0 {
        255
    } else {
        rounded as u8
    }
}

/// Byte-clamp then pack. `a == 0` discards the colour and returns 0.
#[inline]
pub fn pack_rgba(r: f64, g: f64, b: f64, a: f64) -> u32 {
    let a = clamp_byte(a);
    if a == 0 {
        return TRANSPARENT;
    }
    pack_rgba_bytes(clamp_byte(r), clamp_byte(g), clamp_byte(b), a)
}

/// Pack already-clamped bytes.
#[inline]
pub const fn pack_rgba_bytes(r: u8, g: u8, b: u8, a: u8) -> u32 {
    if a == 0 {
        return TRANSPARENT;
    }
    ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32)
}

/// Inverse of [`pack_rgba`]: `[r, g, b, a]`.
#[inline]
pub const fn unpack_rgba(p: u32) -> Rgba {
    [
        (p & 0xff) as u8,
        ((p >> 8) & 0xff) as u8,
        ((p >> 16) & 0xff) as u8,
        ((p >> 24) & 0xff) as u8,
    ]
}

/// `#rgb` / `#rrggbb` / `#rrggbbaa`, case-insensitive, trimmed.
///
/// Returns `None` on anything else, exactly like the JS `parseHex`.
pub fn parse_hex(s: &str) -> Option<Rgba> {
    let t = s.trim().to_ascii_lowercase();
    let h = t.strip_prefix('#')?;
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match h.len() {
        3 => {
            let mut it = h.chars();
            let (r, g, b) = (it.next()?, it.next()?, it.next()?);
            if it.next().is_some() {
                return None;
            }
            Some([expand_nibble(r)?, expand_nibble(g)?, expand_nibble(b)?, 255])
        }
        6 | 8 => {
            let bytes = h.as_bytes();
            let pair = |i: usize| hex_pair(bytes[i], bytes[i + 1]);
            let a = if h.len() == 8 { pair(6)? } else { 255 };
            Some([pair(0)?, pair(2)?, pair(4)?, a])
        }
        _ => None,
    }
}

/// `hh` → byte, JS `parseInt(hh, 16)` equivalent.
#[inline]
fn hex_pair(hi: u8, lo: u8) -> Option<u8> {
    let h = (hi as char).to_digit(16)?;
    let l = (lo as char).to_digit(16)?;
    Some(((h << 4) | l) as u8)
}

/// `#rgb` doubling: `f` → `0xff`.
#[inline]
fn expand_nibble(c: char) -> Option<u8> {
    let v = c.to_digit(16)?;
    Some(((v << 4) | v) as u8)
}

/// Lower-case `#rrggbb`, no alpha.
pub fn to_hex(r: f64, g: f64, b: f64) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        clamp_byte(r),
        clamp_byte(g),
        clamp_byte(b)
    )
}

/// Lower-case `#rrggbbaa`.
pub fn to_hex8(r: f64, g: f64, b: f64, a: f64) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        clamp_byte(r),
        clamp_byte(g),
        clamp_byte(b),
        clamp_byte(a)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // <META - ROLE : parity with tests/core_basic.test.mjs pixel cases | L150-190>
    #[test]
    fn pack_unpack_round_trip() {
        assert_eq!(pack_rgba(255.0, 0.0, 0.0, 255.0), 0xff0000ff);
        let p = pack_rgba(18.0, 52.0, 86.0, 200.0);
        assert_eq!(unpack_rgba(p), [18, 52, 86, 200]);
    }

    #[test]
    fn alpha_zero_normalises_to_transparent() {
        assert_eq!(pack_rgba(255.0, 255.0, 255.0, 0.0), TRANSPARENT);
        assert_eq!(pack_rgba_bytes(255, 255, 255, 0), TRANSPARENT);
        assert_eq!(unpack_rgba(TRANSPARENT), [0, 0, 0, 0]);
    }

    #[test]
    fn channels_clamp_like_uint8_clamped_array() {
        assert_eq!(
            pack_rgba(-20.0, 300.0, 12.0, 255.0),
            pack_rgba_bytes(0, 255, 12, 255)
        );
        assert_eq!(clamp_byte(254.5), 255);
        assert_eq!(clamp_byte(0.5), 1);
        assert_eq!(clamp_byte(f64::NAN), 0);
    }

    #[test]
    fn parse_hex_forms() {
        assert_eq!(parse_hex("#ff0000"), Some([255, 0, 0, 255]));
        assert_eq!(parse_hex("#F00"), Some([255, 0, 0, 255]));
        assert_eq!(parse_hex("  #00ff0080  "), Some([0, 255, 0, 128]));
        assert_eq!(parse_hex("#abcd"), None);
        assert_eq!(parse_hex("ff0000"), None);
        assert_eq!(parse_hex("#gggggg"), None);
        assert_eq!(parse_hex(""), None);
        assert_eq!(parse_hex("#"), None);
    }

    #[test]
    fn to_hex_lowercase_padded() {
        assert_eq!(to_hex(255.0, 0.0, 0.0), "#ff0000");
        assert_eq!(to_hex(0.0, 0.0, 15.0), "#00000f");
        assert_eq!(to_hex8(255.0, 255.0, 255.0, 128.0), "#ffffff80");
    }
}
