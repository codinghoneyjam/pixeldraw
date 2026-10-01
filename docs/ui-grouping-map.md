# UI Grouping Map — Draw Tool V2 (Slice A layout contract, frozen)

> Status: FROZEN. Source-to-target contract for TL-CODE-03..13.
> Later tasks execute moves/merges using only this table and MUST NOT invent new folders.
> This task moves nothing. Docs only, zero source-code change.

## 0. Closed inventory (36 files, verified on disk)

- Root `draw_tool_v2/src/ui/*.js` (31): `app.js`, `collapsible_sections.js`,
  `color_events.js`, `color_palette.js`, `color_palette_data.js`, `color_recent.js`,
  `color_slots.js`, `color_wheel.js`, `dialog_confirm.js`, `dialog_core.js`,
  `dialog_new_doc.js`, `dialog_progress.js`, `dialog_resize.js`, `dialog_restore.js`,
  `dialogs.js`, `dom.js`, `history_buttons.js`, `icons.js`, `menubar.js`,
  `panel_brush.js`, `panel_color.js`, `panel_color_fields.js`, `panel_color_wheel.js`,
  `panel_layers.js`, `panel_options.js`, `panel_refresh.js`, `shortcuts.js`,
  `statusbar.js`, `strings.js`, `tooltip.js`, `view_store.js`.
- Actions `draw_tool_v2/src/ui/actions/*.js` (5): `edit_actions.js`, `file_actions.js`,
  `layer_actions.js`, `tool_actions.js`, `view_actions.js`.
- Entry: `app.js` (loaded by `draw_tool_v2/index.html` entry script).
- Barrel: `dialogs.js` (dialog re-export barrel).

## 1. DOM units (10, from `index.html:14-204`)

| # | Unit | DOM id |
| :--- | :--- | :--- |
| 1 | menubar | `#dt-menubar` |
| 2 | optionbar | `#dt-optionbar` |
| 3 | toolbar | `#dt-toolbar` |
| 4 | left-panels | `#dt-left-panels` |
| 5 | color+brush | `#dt-color-panel` + `#dt-brush-panel` (sections inside left-panels) |
| 6 | canvas-host | `#dt-canvas-host` |
| 7 | right-panels | `#dt-right-panels` |
| 8 | layers | `#dt-layer-panel` (section inside right-panels) |
| 9 | statusbar | `#dt-statusbar` |
| 10 | dialog-root | `#dt-dialog-root` |

## 2. Source-to-target mapping (36/36, each file exactly once)

Convention: DOM-id folder names under `draw_tool_v2/src/ui/`.
`app.js` and `dialogs.js` stay at `src/ui/` root for entry stability.
Shared helpers stay importable at root. No path alias (none exists today;
`app.js` imports each file directly).

| Source file | Target folder | DOM unit |
| :--- | :--- | :--- |
| `src/ui/app.js` | `src/ui/` (root, entry — stays) | entry |
| `src/ui/dialogs.js` | `src/ui/` (root, barrel — stays) | dialog-root (barrel) |
| `src/ui/dom.js` | `src/ui/` (root, shared — stays) | shared |
| `src/ui/strings.js` | `src/ui/` (root, shared — stays) | shared |
| `src/ui/panel_refresh.js` | `src/ui/` (root, shared — stays) | shared |
| `src/ui/collapsible_sections.js` | `src/ui/` (root, shared — stays) | shared |
| `src/ui/tooltip.js` | `src/ui/` (root, shared — stays) | shared |
| `src/ui/menubar.js` | `src/ui/menubar/` | menubar |
| `src/ui/history_buttons.js` | `src/ui/menubar/` | menubar |
| `src/ui/actions/edit_actions.js` | `src/ui/menubar/` | menubar |
| `src/ui/actions/file_actions.js` | `src/ui/menubar/` | menubar |
| `src/ui/actions/tool_actions.js` | `src/ui/menubar/` | menubar |
| `src/ui/actions/view_actions.js` | `src/ui/menubar/` | menubar |
| `src/ui/panel_options.js` | `src/ui/optionbar/` | optionbar |
| `src/ui/icons.js` | `src/ui/toolbar/` | toolbar |
| `src/ui/shortcuts.js` | `src/ui/toolbar/` | toolbar |
| `src/ui/panel_color.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/panel_brush.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/panel_color_fields.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/panel_color_wheel.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/color_wheel.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/color_palette.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/color_palette_data.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/color_recent.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/color_slots.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/color_events.js` | `src/ui/left-panels/` | color+brush |
| `src/ui/view_store.js` | `src/ui/canvas-host/` | canvas-host |
| `src/ui/panel_layers.js` | `src/ui/right-panels/` | layers |
| `src/ui/actions/layer_actions.js` | `src/ui/right-panels/` | layers |
| `src/ui/statusbar.js` | `src/ui/statusbar/` | statusbar |
| `src/ui/dialog_core.js` | `src/ui/dialog-root/` | dialog-root |
| `src/ui/dialog_confirm.js` | `src/ui/dialog-root/` | dialog-root |
| `src/ui/dialog_new_doc.js` | `src/ui/dialog-root/` | dialog-root |
| `src/ui/dialog_progress.js` | `src/ui/dialog-root/` | dialog-root |
| `src/ui/dialog_resize.js` | `src/ui/dialog-root/` | dialog-root |
| `src/ui/dialog_restore.js` | `src/ui/dialog-root/` | dialog-root |

## 3. Folder inventory (frozen — 8 group folders + root)

- `src/ui/` root: `app.js`, `dialogs.js`, `dom.js`, `strings.js`,
  `panel_refresh.js`, `collapsible_sections.js`, `tooltip.js` (7 files).
- `src/ui/menubar/` (6): `menubar.js`, `history_buttons.js`,
  `edit_actions.js`, `file_actions.js`, `tool_actions.js`, `view_actions.js`.
  Rationale: the four non-layer action modules implement the menubar
  `data-action` commands (`file.*`, `edit.*`, `view.*`, tool switching);
  they move with the menubar unit. `layer_actions.js` stays with layers.
- `src/ui/optionbar/` (1): `panel_options.js`.
- `src/ui/toolbar/` (2): `icons.js`, `shortcuts.js`.
- `src/ui/left-panels/` (10): `panel_color.js`, `panel_brush.js`,
  `panel_color_fields.js`, `panel_color_wheel.js`, `color_wheel.js`,
  `color_palette.js`, `color_palette_data.js`, `color_recent.js`,
  `color_slots.js`, `color_events.js`.
- `src/ui/canvas-host/` (1): `view_store.js`.
- `src/ui/right-panels/` (2): `panel_layers.js`, `layer_actions.js`.
- `src/ui/statusbar/` (1): `statusbar.js`.
- `src/ui/dialog-root/` (6): `dialog_core.js`, `dialog_confirm.js`,
  `dialog_new_doc.js`, `dialog_progress.js`, `dialog_resize.js`,
  `dialog_restore.js`.

## 4. Barrel / alias rules

- No path alias is introduced (none exists today).
- `dialogs.js`-style barrel per dialog group: the moved dialog group keeps one
  barrel at `src/ui/dialog-root/` re-exporting the group (same pattern as the
  current root `dialogs.js`, which stays untouched at root for entry stability).
- Shared helpers (`dom.js`, `strings.js`, `panel_refresh.js`,
  `collapsible_sections.js`, `tooltip.js`) stay importable at `src/ui/` root;
  moved files reference them with corrected `../` depth only.
- Moved files MUST preserve their `../core/`, `../io/`, `../model/`,
  `../render/`, `../tools/` relative paths (re-verify `../` depth per move).

## 5. Reused SSOT (cited from plan Section 2, not duplicated)

- `mountColor` (`panel_color.js`), `mountBrush` + `presetColumns` /
  `presetRows` / `paintPreview` / `stepPenSize` (`panel_brush.js`).
- `mountLayers` / `paintThumb` (`panel_layers.js`).
- `mountOptions` / `mountCollapsibleSections`
  (`panel_options.js`, `collapsible_sections.js`).
- `openModal` / `addButton` / `numberField` (`dialog_core.js`).
- `el` / `qs` / `qsa` / `on` / `setHidden` / `clearChildren` (`dom.js`).
- `createColorWheel` + `WHEEL_PX` / `RING_*` / `SV_PX` +
  `rgbToHsv` / `hsvToRgb` / `angleToHue` / `hsvToHex`
  (`panel_color_wheel.js`).
- `normHex` / `resolveColorFields` / `paintColorFields` / `makeSwatchButton`
  (`panel_color_fields.js`).

## 6. Out of scope (later / separate slices)

- Physical moves: TL-CODE-03..10. Merges: Slice C task(s).
- `draw_tool_v2/index.html` entry referrer: owned by TL-CODE-04.
- `draw_tool_v2/src/tools/*`, PenTool, AutosaveStore (G1/G2/G3): FUTURE slices.
- Global referrer sweep outside `src/ui` (Q7 DEFERRED): separate approval.
