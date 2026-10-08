# Draw Tool v2

General-purpose pixel art editor for tile-based 2D assets. Dependency: none.
Pixel-pen raster editor. 32px chunk layers, no antialiasing, zero npm dependencies, no build step.

## Run

```bash
cd draw_tool_v2
python run.py
```

Picks a free loopback port, serves the tool, and opens your browser.
`--port N` to prefer a port, `--no-browser` to just print the URL.
`file://` direct open is **unsupported** (Chromium blocks ES modules over CORS),
which is why `run.py` exists.

## Test

```bash
npm test            # node --test tests/  (Node >= 20, no framework)
npm run parity      # node tools/parity_check.mjs  (115 golden checks)
npm run parity:assets  # node tools/recipe_parity_check.mjs  (39 legacy PNG renders)
npm run layers      # node tools/extract_layers.mjs  (per-layer PNG + Document JSON)
npm run gallery     # node tools/build_layer_gallery.mjs  (34 assets x 3, see below)
node tools/png_cases_check.mjs   # PNG codec cases 5/5
node tools/view_check.mjs        # view-math worked examples
python schema/schema_selfcheck.py  # schema + semantic + flatten parity
```

## Layer gallery (viewer/)

`viewer/index.html` lists the 34 ACTIVE legacy assets
(`asset_work/tidy/numbered/11_*.png` .. `44_*.png`) as triples:
① the draw_tool_v2 **layer units** (PNG + `_drawtool.json`), ② the **PNG rendered
from those layers**, ③ the **legacy PNG**. Each card also bakes a diff map and
reports RGBA parity against the legacy file.

`node tools/build_layer_gallery.mjs` regenerates everything
(`asset_work/layers/gallery/` + `viewer/js/gallery_data.js`) and fails the process
if any of the 34 lacks layers or scores below 0.99. Serve it with
`python run.py` and open `/viewer/`; it also works from `file://`.

## Layout

```text
index.html  style.css  package.json  run.py
src/
  core/    constants.js errors.js events.js pixel.js blend.js chunkstore.js brush.js
           raster/ shape_raster.js raster_brush.js raster_masks.js raster_snap.js
  model/   layer.js document.js commands.js history.js session.js
           settings_validator.js layer_operations.js edit_session.js
  io/      base64.js png.js validate_structural.js validate_semantic.js serialize.js
           export_png.js file_io.js store_idb.js
           idb_schema.js idb_record_builder.js idb_scheduler.js idb_loader.js
  render/  view.js composite.js renderer.js renderer_composite.js renderer_display.js
           background.js grid.js
  tools/   tool_base.js tool_manager.js input_controller.js pen.js eyedropper.js fill.js
           hand.js shape.js shape_geom.js shape_overlay.js
  ui/      app.js dom.js strings.js icons.js panel_*.js color_*.js dialog_*.js
           collapsible_sections.js panel_refresh.js view_store.js menubar.js
           history_buttons.js statusbar.js shortcuts.js tooltip.js
           actions/ file_actions.js edit_actions.js view_actions.js layer_actions.js
                    tool_actions.js
tests/     *.test.mjs  helpers/  fixtures/   (fixtures are normative)
schema/    layer_schema.v2.json document_schema.v2.json
tools/     parity_check.mjs png_cases_check.mjs view_check.mjs png_unique_colors.mjs + gen_*.py
reference/ raster_ref.mjs png_ref.mjs view_ref.mjs (non-normative)
```

`src/`는 계층당 7~36개 파일, 총 85개 `.js`다. 상세 구성과 라인 수는 각 계층 문서
(`docs/core.md`·`model.md`·`io.md`·`render.md`·`tools.md`·`ui.md`)를 따른다.

Headless integration: `tests/e2e_session.test.mjs` drives `Session` via
`ToolManager` + fake `ToolEvent` (H3 1-9: diagonal scenario, hash-verified
undo/redo, serialize determinism, dirty flag, layer export-import, 4x3 shape
undo, no-AA PNG check, locked/hidden reject, small-budget history eviction).

## Known issues (2026-10-01)

- `npm test` fails on Node 24: `node --test tests/` is not accepted as a directory
  argument. The `package.json` script is still unfixed. Run
  `node --test "tests/*.test.mjs"` instead.
- Open code issues (test suite itself is green):
  - `src/core/raster/raster_brush.js` is dead code. It duplicates `brushFootprint`
    and `forEachBresenham` from `src/core/brush.js`, and nothing imports it.
    Left over from the `raster/` split (commit `62f6d0ae`).
  - `rnd` is defined twice: `src/core/blend.js` and `src/core/raster/raster_snap.js`.
  - `src/core/raster/*` throws `RangeError`, but `docs/contract.md` section 2-7
    allows `DrawToolError` only.

## Verification status (2026-10-08)

All gates pass:

| Gate | Result |
|:---|:---|
| `node --test "tests/*.test.mjs"` | 324/324 |
| `node tools/parity_check.mjs` | 201 golden checks |
| `node tools/recipe_parity_check.mjs` | 39/39 legacy renders |
| `node tools/build_layer_gallery.mjs` | 34/34 assets layered, min parity 0.990131 |
| `node tools/png_cases_check.mjs` | 5/5 |
| `node tools/view_check.mjs` | OK |
| `python schema/schema_selfcheck.py` | OK |
