//! Unit tests for the framebuffer ops and the background painter.

use super::background::{paint_background, paint_checker};
use super::framebuffer::Framebuffer;
use super::grid::GridMode;
use crate::constants::{CHECKER_RGBA_A, CHECKER_RGBA_B};

fn px(fb: &Framebuffer, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * fb.width + x) * 4) as usize;
    [fb.rgba[o], fb.rgba[o + 1], fb.rgba[o + 2], fb.rgba[o + 3]]
}

fn fill(fb: &mut Framebuffer, rgba: [u8; 4]) {
    for px in fb.rgba.as_chunks_mut::<4>().0 {
        px.copy_from_slice(&rgba);
    }
}

/// The contract's `rnd`, spelled out so the expectations are hand-computed.
fn rnd_u8(v: f64) -> u8 {
    (v + 0.5).floor() as u8
}

mod cases_01;
mod cases_02;
