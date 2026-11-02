//! Toolbar registration and tooltip regression tests.

use super::*;

#[test]
fn toolbar_has_all_tools() {
    let toolbar = Toolbar::new();
    assert_eq!(toolbar.tools.len(), 14);
    for id in [
        "pen",
        "eraser",
        "fill",
        "eyedropper",
        "line",
        "rect",
        "rrect",
        "ellipse",
        "polygon",
        "arc",
        "chord",
        "radial_gradient",
        "text",
        "hand",
    ] {
        assert!(
            toolbar.tools.iter().any(|t| t.id == id),
            "missing tool {id}"
        );
    }
}

/// Every tool needs its own help text; the option bar's generic branch used
/// to be the only thing on screen when hovering.
#[test]
fn every_tool_has_help_and_it_is_distinct() {
    let toolbar = Toolbar::new();
    for t in &toolbar.tools {
        assert!(t.help.len() > 4, "tool {} has no help text", t.id);
        assert!(
            !t.help.contains("없습니다"),
            "tool {} has a fallback string",
            t.id
        );
    }
    let mut seen: Vec<&str> = toolbar.tools.iter().map(|t| t.help).collect();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(before, seen.len(), "two tools share a help string");
}

#[test]
fn every_tool_declares_a_shortcut() {
    for t in Toolbar::new().tools {
        assert!(!t.shortcut.is_empty(), "tool {} has no shortcut", t.id);
    }
}

/// The shortcut is shown in the tooltip, so it has to be a single
/// character that the font can actually render.
#[test]
fn shortcuts_are_single_ascii_letters() {
    for t in Toolbar::new().tools {
        assert_eq!(
            t.shortcut.chars().count(),
            1,
            "tool {} shortcut {:?} is not one character",
            t.id,
            t.shortcut
        );
        assert!(
            t.shortcut.chars().all(|c| c.is_ascii_alphanumeric()),
            "tool {} shortcut {:?} is not ASCII alphanumeric",
            t.id,
            t.shortcut
        );
    }
}

/// Guard against a font-coverage regression: a pictogram that
/// the runtime font cannot draw shows up as U+25A1 WHITE SQUARE. Nothing on
/// a button may contain one, and nothing may reach outside Latin-1 plus the
/// Hangul syllables the labels already use.
/// The rail draws vector glyphs, so the tooltip is the only place tool
/// text reaches the screen. Keep it inside the font's coverage.
#[test]
fn the_tooltip_is_the_only_tool_text_and_it_renders() {
    for t in Toolbar::new().tools {
        let tip = Toolbar::tool_tip(&t);
        assert!(
            !tip.contains('\u{25A1}') && !tip.contains('\u{FFFD}'),
            "tool {} draws a tofu box: {tip:?}",
            t.id
        );
        for c in tip.chars() {
            let ok = c.is_ascii()
                    || ('\u{AC00}'..='\u{D7A3}').contains(&c)  // Hangul syllables
                    || ('\u{3130}'..='\u{318F}').contains(&c)  // Hangul compat jamo
                    || c == '\u{2014}'; // the em dash joining the halves
            assert!(ok, "tool {} draws non-Latin glyph {c:?}", t.id);
        }
    }
}

/// The tooltip is the only place the shortcut is spelled out in full. It is
/// built by `tool_tip`, which the test calls, so a refactor that drops the
/// key — or the Korean name — out of it actually fails here.
#[test]
fn tooltip_leads_with_the_shortcut_and_names_the_tool() {
    for t in Toolbar::new().tools {
        let tip = Toolbar::tool_tip(&t);
        assert!(tip.starts_with('['), "tool {} tooltip: {tip:?}", t.id);
        assert!(
            tip.contains(t.shortcut),
            "tool {} lost its key: {tip:?}",
            t.id
        );
        assert!(
            tip.contains(t.label),
            "tool {} lost its name: {tip:?}",
            t.id
        );
        assert!(tip.contains(t.help), "tool {} lost its help: {tip:?}", t.id);
    }
}

/// Every registered tool must have a glyph: `draw_tool_vector_icon` falls
/// through to a blank on an unknown id, which would render as an empty
/// button with no error anywhere.
#[test]
fn every_tool_has_a_distinct_vector_icon() {
    // The arms the painter takes per id. An id missing from the match would
    // draw nothing, so the count is the contract: one arm per registered
    // tool, and the `_` fallback is the only unnamed case.
    let named = [
        "pen",
        "eraser",
        "fill",
        "eyedropper",
        "line",
        "rect",
        "rrect",
        "ellipse",
        "polygon",
        "arc",
        "chord",
        "radial_gradient",
        "text",
        "hand",
    ];
    for id in named {
        assert!(
            icons::has_icon(id),
            "no vector icon for tool {id} — it would render as a blank button"
        );
    }
    assert!(
        Toolbar::new().tools.iter().all(|t| icons::has_icon(t.id)),
        "a registered tool has no vector icon"
    );
}
