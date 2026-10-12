//! `src/tools/cursor_state.js` — the `outside > panning > space > tool` ladder.
//!
//! Pure decision, DOM-free by contract (`docs/contract.md` §4): the host turn of
//! the resolved [`CursorKind`] into a CSS cursor is the adapter's job (W8).

use crate::tools::CursorKind;

/// The four fixed outcomes of the ladder.
pub const DEFAULT_CURSOR: CursorKind = CursorKind::Default;
pub const PAN_CURSOR: CursorKind = CursorKind::Grabbing;
pub const SPACE_CURSOR: CursorKind = CursorKind::Grab;

/// `readCursorState`'s four inputs. `tool_cursor: None` is the JS "the active
/// tool is unregistered / threw" fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorInputs {
    pub inside: bool,
    pub panning: bool,
    pub space_down: bool,
    pub tool_cursor: Option<CursorKind>,
}

impl Default for CursorInputs {
    /// `readCursorState({})` — an absent controller counts as present and still.
    fn default() -> Self {
        Self {
            inside: true,
            panning: false,
            space_down: false,
            tool_cursor: None,
        }
    }
}

/// `resolveCursor` — outside beats panning beats space beats the tool.
pub fn resolve_cursor(c: CursorInputs) -> CursorKind {
    if !c.inside {
        return DEFAULT_CURSOR;
    }
    if c.panning {
        return PAN_CURSOR;
    }
    if c.space_down {
        return SPACE_CURSOR;
    }
    c.tool_cursor.unwrap_or(DEFAULT_CURSOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ported from `tests/tools_manager.test.mjs` §cursor state machine — the
    /// whole truth table, because it is cheap and pure.
    #[test]
    fn ladder_truth_table() {
        let tool = Some(CursorKind::Crosshair);
        let rows = [
            (
                CursorInputs {
                    inside: true,
                    panning: false,
                    space_down: false,
                    tool_cursor: tool,
                },
                CursorKind::Crosshair,
                "tool wins when nothing else is active",
            ),
            (
                CursorInputs {
                    inside: true,
                    panning: false,
                    space_down: true,
                    tool_cursor: tool,
                },
                SPACE_CURSOR,
                "space beats the tool",
            ),
            (
                CursorInputs {
                    inside: true,
                    panning: true,
                    space_down: true,
                    tool_cursor: tool,
                },
                PAN_CURSOR,
                "panning beats space",
            ),
            (
                CursorInputs {
                    inside: false,
                    panning: true,
                    space_down: true,
                    tool_cursor: tool,
                },
                DEFAULT_CURSOR,
                "outside beats everything",
            ),
            (
                CursorInputs {
                    inside: false,
                    ..CursorInputs::default()
                },
                DEFAULT_CURSOR,
                "outside alone",
            ),
            (
                CursorInputs {
                    inside: true,
                    ..CursorInputs::default()
                },
                DEFAULT_CURSOR,
                "a missing tool cursor falls back",
            ),
            (
                CursorInputs::default(),
                DEFAULT_CURSOR,
                "an empty state object is all defaults",
            ),
        ];
        for (inputs, want, why) in rows {
            assert_eq!(resolve_cursor(inputs), want, "{why}");
        }
    }

    /// `Some(CursorKind::Default)` is the JS "tool returned an empty string".
    #[test]
    fn an_explicit_default_tool_cursor_is_not_a_fallback() {
        let c = CursorInputs {
            tool_cursor: Some(CursorKind::Default),
            ..CursorInputs::default()
        };
        assert_eq!(resolve_cursor(c), DEFAULT_CURSOR);
    }
}
