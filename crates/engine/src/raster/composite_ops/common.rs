//! Pixel helpers shared by every read-back operation.
//!
//! They live apart from the operations because both `visor` and `sheen` need
//! them and neither owns the other.

use crate::pixel::Rgba;

/// Alpha is the top byte of the packed `a<<24|b<<16|g<<8|r` pixel.
#[inline]
pub(crate) fn alpha_of(px: u32) -> u8 {
    ((px >> 24) & 0xff) as u8
}

#[inline]
pub(crate) fn channels_of(px: u32) -> Rgba {
    [
        (px & 0xff) as u8,
        ((px >> 8) & 0xff) as u8,
        ((px >> 16) & 0xff) as u8,
        ((px >> 24) & 0xff) as u8,
    ]
}

/// JS `Math.round`: half toward +∞, never zero-away-from-zero.
#[inline]
pub(crate) fn round_half_up(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}
