# Native UI Layout Audit

## Summary

The main layout defect is panel declaration order in `crates/app/src/ui/mod.rs`.
The option bar is declared after the left tool rail and left sidebar, and the
status bar is declared after the right sidebar. In egui, each panel consumes
space from the remaining rectangle when it is declared. As a result, those
horizontal bars are restricted to the width left over for the canvas instead
of spanning the window.

This is consistent with the supplied screenshot: the option controls begin to
the right of the left sidebar, while the status readout is confined to the
canvas column. The most useful first change is structural, not cosmetic.

## What The Code Does Today

`render_ui()` currently declares panels in this order:

1. Menubar (`top`)
2. Tool rail (`left`)
3. Left panels (`left`)
4. Option bar (`top`)
5. Right panels (`right`)
6. Status bar (`bottom`)
7. Dialogs and transient overlays

The canvas `CentralPanel` is declared afterward in `shell.rs`.

The panels lay out into the remaining area. Therefore:

- The menubar is full width because it is first.
- The tool rail and left panels take width before the option bar is declared.
- The option bar only spans the remaining center/right portion.
- The right sidebar then takes its width from that remaining area.
- The status bar is declared last among the layout panels, so it spans only
  what remains between the side panels.
- The central canvas receives the final rectangle after all of those cuts.

The JS layout provides a useful intended contract: `#dt-menubar`,
`#dt-optionbar`, and `#dt-statusbar` span all columns; the rail, sidebars, and
canvas occupy the workspace row between the option and status bars. See
`style_base.css` and `index.html`.

## Other Space Constraints

- The Rust shell starts at 1280 × 820 logical points and allows a minimum of
  900 × 560 (`crates/app/src/shell.rs`).
- The tool rail is pinned to 44 points — the JS `#dt-app` toolbar column (a
  36-point button plus 4-point sides, `#dt-toolbar` gap 4px) — instead of
  egui's 96-point left-panel default minimum.
- The left and right panels set explicit default and minimum widths in
  `render_ui()`; the left panel is content-sized (a wide HEX field pushes it
  past its 256-point default).
- Both sidebars are vertically scrollable, and the left sidebar stacks color,
  palette, recent colors, and brush controls. This is a lot of content for the
  560-point minimum window height.
- The JS layout explicitly models full-width bars and resizable sidebars. The
  Rust layout currently has resizable sidebars, but no documented width budget
  or narrow-window collapse behavior.

## Recommended Layout Contract

Declare full-width panels before side panels, then leave `CentralPanel` last:

1. Menubar (`top`)
2. Option bar (`top`)
3. Status bar (`bottom`)
4. Tool rail (`left`)
5. Left panels (`left`)
6. Right panels (`right`)
7. Dialogs and transient overlays
8. Canvas `CentralPanel` in the shell

This makes both horizontal bars span the whole window and gives the canvas the
center rectangle between the sidebars. Keep the rail before the two sidebars
so it remains the leftmost strip. Keep the canvas `CentralPanel` after all
other panels.

## Width And Density Recommendations

Use a deliberate width budget rather than relying on widget content to size
the sidebars. The current Rust implementation sets:

- Tool rail: pinned to 44 points outer (a 36-point button plus 4-point sides) —
  the JS `#dt-app` toolbar column width, pinned with `size_range(44..=44)` so
  egui's default 96-point minimum never applies.
- Left color/tools panel: 256-point default, 180-point minimum.
- Right layers panel: 282-point default, 200-point minimum.
- Either side panel can grow up to 440 points, subject to the opposite panel's
  previous-frame outer width and the shared canvas budget.
- Side-panel size limits include their frame margins, per egui 0.36.2.
- At the 900-point minimum, the test reserves 84 points for the rail and
  gutters plus 220 points for the canvas. The remaining shared sidebar budget
  is 596 points. For example, with the opposite panel at its default size the
  left can grow to 314 points and the right to 340; if one panel shrinks, the
  other can use the freed space up to 440 points.
- Panel widths are not saved across application restarts. Adding width
  persistence requires a settings-schema change and is not part of the current
  layout patch.
- Sidebars remain draggable; their maxima intentionally preserve canvas space
  instead of letting either panel grow without a coordinated width budget.
- Keep the option bar compact and tool-specific. Let long option groups wrap
  or scroll horizontally rather than shrinking the canvas or moving controls
  into sidebars.
- Keep the status bar full-width, with stable left/center/right groups. At
  narrow widths, hide lower-priority help text before coordinates or zoom.
- Keep vertical scrolling in the sidebars. Consider collapsing secondary
  sections (recent colors and brush presets) by default when the window is
  short.

The declared panel widths are logical egui points and include their frame
margins. The 84-point rail/gutter reserve is a conservative budget assumption;
the native-window drag check in `docs/native_smoke.md` is still needed to
confirm the rendered geometry at 900 × 560.

## Suggested Verification

1. Reorder only the panel declarations and capture the app at 1280 × 820.
2. Confirm the option and status bars touch both window edges, while the
   canvas remains centered between the sidebars.
3. Repeat at the 900 × 560 minimum size; confirm no controls overlap and the
   canvas remains usable.
4. Drag both sidebar separators and verify the canvas shrinks/grows without
   changing the horizontal bars' width.
5. Compare with the static wireframe: [native_ui_layout_compare.html](native_ui_layout_compare.html).

## Code Anchors

- Panel declaration order: `crates/app/src/ui/mod.rs`, `render_ui()`.
- Canvas as the final center panel: `crates/app/src/shell.rs`, `PixeldrawApp::ui()`.
- Window dimensions: `crates/app/src/shell.rs`, `WINDOW_SIZE` and
  `WINDOW_MIN_SIZE`.
- JS full-width layout reference: `style_base.css`, `#dt-app` and the
  `#dt-menubar, #dt-optionbar, #dt-statusbar` rule.