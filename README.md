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
node --test "tests/*.test.mjs"   # node --test tests/  (Node >= 20, no framework)
npm run parity      # node tools/parity_check.mjs  (201 golden checks)
npm run parity:assets  # node tools/recipe_parity_check.mjs  (39 legacy PNG renders)
npm run layers      # node tools/extract_layers.mjs  (per-layer PNG + Document JSON)
npm run gallery     # node tools/build_layer_gallery.mjs  (34 assets x 3, see below)
node tools/png_cases_check.mjs   # PNG codec cases 5/5
node tools/view_check.mjs        # view-math worked examples
node tests/tool_integrity_check.mjs  # post-cleanup gate: exports, line counts, link check
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
index.html  package.json  run.py  split_serialize.py
style_base.css  style_polish.css  style_juicy.css  style_picker.css
src/
  core/     constants.js errors.js events.js pixel.js blend.js chunkstore.js
            raster/  (arc atlas composite_ops gradient polygon raster_brush
                      raster_masks raster_snap segment shape_raster text_mask)
  features/ 툴 기능 단위 피처 패키지
            viewport/ view.js (줌·팬 수학) view_store.js
            document/ document.js history.js session.js settings_validator.js
            layers/   layer.js layer_operations.js commands.js commands_pixel.js
                      edit_session.js layer_actions.js panel_layers.js
            pen/      pen.js (pen+eraser) brush.js panel_brush.js
            shape/    shape.js shape_geom.js shape_overlay.js shape_render.js
                      shape_pending.js shape_keys.js
            fill/     fill.js
            eyedropper/ eyedropper.js
            hand/     hand.js
            color/    panel_color.js panel_color_fields.js panel_color_wheel.js
                      color_palette.js color_recent.js color_slots.js color_events.js
  io/       base64.js png.js validate.js validate_structural.js validate_semantic.js
            serialize.js (래스터 + 배럴) serialize_vector.js serialize_recipe.js
            export_png.js file_io.js store_idb.js
            idb_schema.js idb_record_builder.js idb_scheduler.js idb_loader.js
  render/   composite.js renderer.js renderer_composite.js renderer_display.js
            background.js grid.js
  tools/    tool_base.js tool_manager.js input_controller.js
            pointer_bindings.js keyboard_bindings.js pointer_event.js wheel_accumulator.js
            cursor_state.js
  ui/       app.js
            actions/ action_map.js edit_actions.js file_actions.js
            menubar/ menubar.js menubar_wiring.js history_buttons.js
            optionbar/ panel_options.js
            toolbar/ icons.js shortcuts.js toolbar_wiring.js
            statusbar/ statusbar.js
            dialogs/ dialog_core.js dialog_new_doc.js dialog_resize.js
            shared/  dom.js strings.js tooltip.js collapsible_sections.js
                     panel_refresh.js dialogs.js storage.js toast.js
            (도구별 액션은 피처로 이감: viewport/view_actions.js·
             color/color_actions.js·pen/brush_actions.js·layers/layer_actions.js)
tests/     *.test.mjs  helpers/  fixtures/   (fixtures are normative)
schema/    layer_schema.v2.json document_schema.v2.json
tools/     parity_check.mjs png_cases_check.mjs view_check.mjs png_unique_colors.mjs + gen_*.py
           recipe/  (transpile.mjs render_tile.js schema.mjs recipe_converter.mjs ...)
reference/ raster_ref.mjs png_ref.mjs view_ref.mjs (non-normative)
```

피처 기반 재편(2026-10-08)으로 모델 계층 `src/model/`은 사라지고 `features/document/`와
`features/layers/`로 갈라졌다. 도구 구현체(`pen`·`shape`·`fill`·`eyedropper`·`hand`)와
패널(`panel_color*`·`panel_brush`·`panel_layers`), 뷰(`view.js`·`view_store.js`)도 각 피처로
옮겨갔으므로 `src/tools/`에는 입력 플럼빙만, `src/render/`에는 합성·표시만 남았다.
`src/`는 계층당 6~36개 파일, 총 **104개 `.js`**다. 상세 구성과 라인 수는 각 계층 문서
(`docs/core.md`·`model.md`·`io.md`·`render.md`·`tools.md`·`ui.md`)를 따른다.

Headless integration: `tests/e2e_session.test.mjs` drives `Session` via
`ToolManager` + fake `ToolEvent` (H3 1-9: diagonal scenario, hash-verified
undo/redo, serialize determinism, dirty flag, layer export-import, 4x3 shape
undo, no-AA PNG check, locked/hidden reject, small-budget history eviction).

## Known issues

- `npm test` fails on Node 24: `node --test tests/` is not accepted as a directory
  argument. The `package.json` script is still unfixed. Run
  `node --test "tests/*.test.mjs"` instead.

## Verification status (2026-10-09)

All gates pass:

| Gate | Result |
|:---|:---|
| `node --test "tests/*.test.mjs"` | 372/372 |
| `node tools/parity_check.mjs` | 201 golden checks |
| `node tools/recipe_parity_check.mjs` | 39/39 legacy renders |
| `node tools/build_layer_gallery.mjs` | 34/34 assets layered, min parity 0.990131 |
| `node tools/png_cases_check.mjs` | 5/5 |
| `node tools/view_check.mjs` | OK |
| `python schema/schema_selfcheck.py` | OK |
| `node tests/tool_integrity_check.mjs` | 34/34 |
