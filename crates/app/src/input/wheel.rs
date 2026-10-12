//! `src/tools/wheel_accumulator.js` — DOM-free wheel delta → integer steps.
//!
//! No cursor functions and no view math here (TL-DRAW-04 Slice D): the pure
//! numeric half, so it stays testable without a host.

/// DOM `deltaMode`: 0 = px, 1 = line, 2 = page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WheelMode {
    #[default]
    Pixel,
    Line,
    Page,
}

/// One step's worth of px (`wheel_accumulator.js` L45 hardcodes 60; it is not
/// registered in `docs/contract.md` §1 yet).
pub const WHEEL_PX_PER_STEP: f64 = 60.0;

/// One `deltaMode === 1` unit (`wheel_accumulator.js` L33 hardcodes 16).
const WHEEL_LINE_PX: f64 = 16.0;

/// `WheelAccumulator` — residue accumulates until a whole step is reached.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelAccumulator {
    residue: f64,
    dir: f64,
}

impl Default for WheelAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl WheelAccumulator {
    pub fn new() -> Self {
        Self {
            residue: 0.0,
            dir: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.residue = 0.0;
        self.dir = 0.0;
    }

    pub fn residue(&self) -> f64 {
        self.residue
    }

    /// Accumulate a vertical wheel delta into `-step` units.
    ///
    /// Returns a **negative** count for a downward (positive) delta, because the
    /// caller feeds the result straight into `step_zoom`, which zooms in on
    /// positive directions.
    pub fn feed(&mut self, delta_y: f64, delta_mode: WheelMode, page_h: f64) -> i32 {
        let mut px = delta_y;
        if !px.is_finite() || px == 0.0 {
            return 0;
        }
        match delta_mode {
            WheelMode::Line => px *= WHEEL_LINE_PX,
            // JS `Number(pageHeight) || 0`: a missing page height contributes 0.
            WheelMode::Page => {
                px *= if page_h.is_finite() { page_h } else { 0.0 };
            }
            WheelMode::Pixel => {}
        }
        if px == 0.0 {
            return 0;
        }
        let dir = px.signum();
        // A direction reversal discards the residue instead of summing against
        // it, otherwise one flip-back overshoots by up to a full step.
        if self.dir != 0.0 && dir != self.dir {
            self.residue = px;
        } else {
            self.residue += px;
        }
        self.dir = dir;
        let steps = (self.residue / WHEEL_PX_PER_STEP).trunc();
        if steps != 0.0 {
            self.residue -= steps * WHEEL_PX_PER_STEP;
            return -steps as i32;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from `tests/tools_manager.test.mjs` §WheelAccumulator and
    // `tests/tool_integrity_check.mjs` §2.
    #[test]
    fn accumulates_small_deltas_until_a_whole_step() {
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(-30.0, WheelMode::Pixel, 0.0), 0);
        assert_eq!(w.feed(-30.0, WheelMode::Pixel, 0.0), 1);
    }

    #[test]
    fn direction_flip_discards_the_residue() {
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(-40.0, WheelMode::Pixel, 0.0), 0);
        assert_eq!(w.feed(40.0, WheelMode::Pixel, 0.0), 0);
        assert_eq!(w.feed(40.0, WheelMode::Pixel, 0.0), -1);
    }

    #[test]
    fn delta_mode_one_and_two() {
        let mut a = WheelAccumulator::new();
        assert_eq!(a.feed(-4.0, WheelMode::Line, 0.0), 1); // -64px => +1
        let mut b = WheelAccumulator::new();
        assert_eq!(b.feed(1.0, WheelMode::Page, 100.0), -1); // +100px => -1
    }

    #[test]
    fn whole_and_half_steps_and_the_threshold() {
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(120.0, WheelMode::Pixel, 0.0), -2);
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(60.0, WheelMode::Pixel, 0.0), -1);
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(30.0, WheelMode::Pixel, 0.0), 0);
        // 59.9 on a fresh accumulator is the last value that still rounds
        // down to nothing (below one 60px step).
        let mut fresh = WheelAccumulator::new();
        assert_eq!(fresh.feed(59.9, WheelMode::Pixel, 0.0), 0);
    }

    #[test]
    fn non_finite_and_zero_deltas_are_ignored() {
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(f64::NAN, WheelMode::Pixel, 0.0), 0);
        assert_eq!(w.feed(f64::INFINITY, WheelMode::Pixel, 0.0), 0);
        assert_eq!(w.feed(0.0, WheelMode::Pixel, 0.0), 0);
    }

    #[test]
    fn page_mode_without_a_page_height_contributes_nothing() {
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(3.0, WheelMode::Page, 0.0), 0);
        assert_eq!(w.feed(3.0, WheelMode::Page, f64::NAN), 0);
    }

    #[test]
    fn reset_clears_the_residue() {
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(30.0, WheelMode::Pixel, 0.0), 0);
        w.reset();
        assert_eq!(w.residue(), 0.0);
        assert_eq!(w.feed(30.0, WheelMode::Pixel, 0.0), 0);
    }

    #[test]
    fn residue_survives_a_partial_step() {
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(90.0, WheelMode::Pixel, 0.0), -1);
        assert_eq!(w.residue(), 30.0);
        assert_eq!(w.feed(30.0, WheelMode::Pixel, 0.0), -1);
    }

    #[test]
    fn reversal_from_a_partial_residue() {
        let mut w = WheelAccumulator::new();
        assert_eq!(w.feed(30.0, WheelMode::Pixel, 0.0), 0);
        assert_eq!(w.feed(-120.0, WheelMode::Pixel, 0.0), 2);
        assert_eq!(w.residue(), 0.0);
    }
}
