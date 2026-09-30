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
node tools/png_cases_check.mjs   # PNG codec cases 5/5
node tools/view_check.mjs        # view-math worked examples
python schema/schema_selfcheck.py  # schema + semantic + flatten parity
```

## Layout

```text
index.html  style.css  package.json  run.py
src/
  core/    constants.js errors.js events.js pixel.js blend.js chunkstore.js brush.js shape_raster.js
  model/   layer.js document.js commands.js history.js session.js
  io/      base64.js png.js validate.js serialize.js export_png.js file_io.js store_idb.js
  render/  view.js composite.js renderer.js (+ background.js grid.js)
  tools/   tool_base.js tool_manager.js input_controller.js pen.js eyedropper.js fill.js shape.js
  ui/      app.js dom.js strings.js icons.js panel_*.js statusbar.js dialogs.js shortcuts.js
tests/     *.test.mjs  helpers/  fixtures/   (fixtures are normative)
schema/    layer_schema.v2.json document_schema.v2.json
tools/     parity_check.mjs png_cases_check.mjs view_check.mjs png_unique_colors.mjs + gen_*.py
reference/ raster_ref.mjs png_ref.mjs view_ref.mjs (non-normative)
```

Headless integration: `tests/e2e_session.test.mjs` drives `Session` via
`ToolManager` + fake `ToolEvent` (H3 1-9: diagonal scenario, hash-verified
undo/redo, serialize determinism, dirty flag, layer export-import, 4x3 shape
undo, no-AA PNG check, locked/hidden reject, small-budget history eviction).
