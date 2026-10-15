//! Hex reading and writing for the colour panel.
//!
//! Split out of the parent `color.rs` together with the HSV maths, so the file
//! that draws the wheel holds widgets. Three shapes, because the callers want
//! three things: the slots want an `egui::Color32`, the channel fields want the
//! raw `[u8; 4]`, and everything wants the canonical `#rrggbb[aa]` spelling
//! back out again (JS `normHex`).
use egui::Color32;

/// Canonical hex with alpha only when translucent (JS `normHex`).
pub(crate) fn to_hex_full(rgba: [u8; 4]) -> String {
    if rgba[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2])
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            rgba[0], rgba[1], rgba[2], rgba[3]
        )
    }
}

/// `#rrggbb` / `#rrggbbaa` as bytes; anything else is `None`. The channel fields
/// rebuild the other channels from this, so they need the bytes, not a colour.
pub(crate) fn parse_hex(hex: &str) -> Option<[u8; 4]> {
    pixel_engine::pixel::parse_hex(hex)
}

/// Same parse, as an `egui::Color32` for the slots and the swatch painters.
pub(crate) fn to_color32(hex: &str) -> Option<Color32> {
    let [r, g, b, a] = pixel_engine::pixel::parse_hex(hex)?;
    Some(Color32::from_rgba_unmultiplied(r, g, b, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_spelling_drops_the_alpha_when_opaque() {
        assert_eq!(to_hex_full([0x12, 0x34, 0x56, 255]), "#123456");
        assert_eq!(to_hex_full([0x12, 0x34, 0x56, 0x80]), "#12345680");
    }

    #[test]
    fn parse_hex_round_trips_through_the_formatter() {
        for hex in ["#123456", "#12345680"] {
            let rgba = parse_hex(hex).expect(hex);
            assert_eq!(&to_hex_full(rgba), hex);
        }
        assert!(parse_hex("nope").is_none());
    }

    #[test]
    fn to_color32_matches_the_engine_parse() {
        assert_eq!(to_color32("#ff0000"), Some(Color32::from_rgb(255, 0, 0)));
        assert!(to_color32("").is_none());
    }
}
