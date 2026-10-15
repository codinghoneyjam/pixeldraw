//! The optionbar seam onto tool-local options.
//!
//! Tools own their own settings (JS: each feature module owns its block), so
//! the bar reaches them through `Tool::as_any_mut` rather than the shared
//! `ToolOptions`, which only carries the pen size the whole app reads.
use crate::session_glue::AppState;

/// JS `toolManager.get(id)` through the native downcast seam
/// (`Tool::as_any_mut`): tool-local options live on the structs, not in the
/// shared `ToolOptions`, exactly like the JS feature modules own their blocks.
pub(crate) fn tool_mut<'a, T: 'static>(app: &'a mut AppState, id: &str) -> Option<&'a mut T> {
    app.tools
        .get_mut(id)
        .and_then(|t| t.as_any_mut().downcast_mut::<T>())
}

/// JS `Math.round((a + b) / 2)` for the polygon midpoint append: halves round
/// toward +∞ unlike Rust's truncating integer division (`(1 + 2) / 2` is 1
/// in Rust, 2 in JS).
pub(crate) fn mid_round_half_up(a: i32, b: i32) -> i32 {
    ((a as i64 + b as i64) as f64 / 2.0 + 0.5).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midpoint_rounds_halves_toward_positive_infinity() {
        assert_eq!(mid_round_half_up(1, 2), 2);
        assert_eq!(mid_round_half_up(0, 0), 0);
        assert_eq!(mid_round_half_up(-1, 0), 0);
        assert_eq!(mid_round_half_up(-3, -2), -2);
        assert_eq!(mid_round_half_up(3, 4), 4);
    }
}
