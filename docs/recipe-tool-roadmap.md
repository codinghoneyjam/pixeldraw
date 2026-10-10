# Recipe Commands And Interactive Tools

## Coverage

Recipe commands in `tools/recipe/render_tile.js` do not all represent user-facing tools. Some are primitive drawing operations, some are compact recipe macros, and some are layer-composition operations.

Already available through existing tools:

- `rect` / `rectangle`, `rounded_rect`, `circle` / `ellipse`, `line`, and `polygon` map to the existing shape tools.
- `pixel` is covered by the pen at a one-pixel brush size.
- `erase` is related to the eraser, but the recipe's arbitrary polygon erase is not a dedicated interaction.
- `lines` and `spokes` are recipe macros. Their output can be approximated with repeated line-tool operations, but they do not have dedicated interactive controls.

Now available as interactive tools:

- `text`: type a string in the option bar, choose one of the baked bitmap fonts, set an optional outline width, then click to stamp. The primary color fills and the secondary color outlines. Only characters present in the selected baked font are supported.
- `arc` and `chord`: drag an ellipse extent, then set start/end angles and stroke width. Chords can independently fill and outline.
- `radial_gradient`: drag from center to radius. The primary color is the inner stop and the secondary color is the outer stop; the outer edge can fade to transparent or remain solid.

## Follow-Up Candidates

- `alpha` is a recipe wrapper for applying opacity to a shape. Shape tools already accept alpha-bearing paint colors, so a dedicated alpha tool is not necessary; exposing opacity directly in shape options may be useful later.
- Polygon `erase` could become a destructive polygon-selection/erase workflow, but it needs selection feedback and cancellation semantics rather than another paint brush.
- `clip_alpha` modifies existing layer coverage and should be designed as a layer operation with preview and undo, not as a normal draw tool.
- `visor`, `sheen`, and `hud_brackets` are specialized compositing recipes. They are better represented as reusable effects or recipe generators than general-purpose canvas tools.
- `lines` and `spokes` could be added later as parameterized shape tools if repeated geometric construction is a common editing workflow.
- The radial-gradient tool currently exposes a two-stop profile. The recipe rasterizer still supports arbitrary multi-stop and non-monotonic profiles; an editable stop list is a separate UI slice.

## Implementation Slices

1. Text stamping: browser-load the committed glyph bitmaps, expose text/font/outline controls, and record each placement as one undoable edit.
2. Arc and chord: map drag bounds and angle/width options to the existing parity-tested masks; chord fill and outline are independently selectable.
3. Radial gradient: map drag center/radius and the two palette colors to the existing radial-gradient rasterizer, with transparent or solid outer coverage.

Each slice has a focused test for tool registration and pixel/history behavior. The existing recipe rasterizer remains the source of rendering semantics; these tools do not fork its masks.