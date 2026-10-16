# Pixeldraw UI/UX Analysis for Rust Port

## Overview

This report provides a detailed analysis of the JavaScript/TypeScript UI/UX implementation in the Pixeldraw application, intended as a reference for Rust porting. The application is built on **Tauri** with a React-like vanilla JS frontend, using an event-driven architecture across the entire stack.

---

## 1. Application Architecture

### 1.1 Boot Sequence (`src/ui/app.js`)
The application follows a strict boot order (lines 51-191):
1. **Settings & recent colors** - Restore saved settings and recently used colors
2. **Autosave restore** - Prompt user to restore from auto-save or start new document
3. **Document creation** - Create default or restored document
4. **Tool Manager & Renderer** - Initialize tool management and canvas rendering
5. **Input Controller** - Set up pointer and keyboard input handling
6. **Menubar & Toolbar** - Wire main menu and toolbar buttons
7. **Panels** - Mount collapsible panels (color, brush, layers, options)
8. **Settings persistence** - Attach storage listeners

### 1.2 Event System (`src/core/events.js`)
A centralized pub/sub event system with these event types:
- `document-replaced` - Document loaded/replaced (autosave, new doc)
- `layers-changed` - Layer modifications
- `pixels-changed` - Pixel-level changes
- `history-changed` - Undo/redo state changes
- `settings-changed` - Settings modifications
- `status-message` - Transient status messages
- `tool-state` - Tool state changes (pending geometry)

---

## 2. User Interface Components

### 2.1 Toolbar (`src/ui/toolbar/`)
- **Purpose**: Tool selection and keyboard shortcuts
- **Structure**: Buttons with `data-tool` attribute, each mapped to an icon
- **Interaction**:
  - Click → sets `session.settings.activeTool`
  - `aria-pressed` state syncs with active tool
  - Icons are SVG or Korean character fallbacks (`src/ui/toolbar/icons.js`)
- **Tool mapping**: pen, eraser, fill, eyedropper, line, rect, rrect, ellipse, polygon, arc, chord, radial_gradient, text, hand

### 2.2 Menubar (`src/ui/menubar/`)
- **Pattern**: Hover + click to open menus, 220ms delayed close
- **Accessibility**: `aria-expanded` on triggers, `Escape` to close all
- **Structure**: `.dt-menu` wraps containing `.dt-menu` panels
- **History buttons**: Undo/redo buttons in menubar, disabled when busy or at history limits

### 2.3 Option Bar / Panels (`src/ui/optionbar/`, `src/ui/statusbar/`, `src/ui/menubar/`)
- **Option Bar** (`src/ui/optionbar/panel_options.js`): 
  - Collapsible sections visible based on active tool
  - Sections: `view`, `shape`, `arc`, `gradient`, `text`
  - Each section owns its controls; shell only handles visibility
  - Pen size controls: number input, range slider, grid of preset buttons (4-column grid)
  - Wheel support for pen size adjustment
  - Real-time preview of brush footprint

- **Status Bar** (`src/ui/statusbar/statusbar.js`):
  - **Zoom level**: `%` readout (e.g., `100%`)
  - **Canvas size**: `512 × 512 px (8 × 8 타일)`
  - **Cursor position**: Pixel coordinates `x, y px` and grid cell `셀 cx, cy (64px) · 타일 tx, ty (32px)`
  - **Active tool**: Text label from `STRINGS.tools`
  - **Dirty indicator**: `●` when unsaved changes
  - Message timers: 4s for info, 8s for warn, immediate for error

- **Layer Panel** (`src/ui/layers/panel_layers.js`):
  - List of layers with thumbnails (32×32 pixels)
  - Eye button to show/hide layer
  - Lock button to lock/unlock layer
  - Double-click to rename
  - Up/down buttons to reorder
  - Merge down button
  - Opacity slider with number input
  - Blend mode dropdown (normal, multiply, screen, overlay, darken, lighten)

- **Viewport Options** (`src/features/viewport/panel_view_options.js`):
  - Grid mode: off/unit/tile/pixel cycle (Ctrl+')
  - Zoom select: preset levels [0.25, 0.5, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32]
  - Fit to window, Actual size (100%)

### 2.4 Dialog System (`src/ui/dialogs/`)
- **Modal shell** (`src/ui/dialogs/dialog_core.js`):
  - Overlay + centered dialog box
  - `Escape` to close, `Tab` cycles focus between focusables
  - Pointer down on overlay closes dialog
  - Focus management: first focusable element receives focus on open

- **Specific dialogs**:
  - **New Document** (`dialog_new_doc.js`): Tile-based dimensions, preset sizes, background color picker, name input
  - **Resize Canvas** (`dialog_resize.js`): Width/height with `32px` unit constraint
  - **Confirm Discard Changes** (`confirmDiscardChanges()`): "Unsaved changes exist. Discard?"
  - **Layer Export Format** (`showLayerExportFormatDialog()`): Chunks vs commands JSON format
  - **Auto-save Restore** (`showRestoreDialog()`): Restore or discard saved document

### 2.5 Color Panel (`src/features/color/panel_color.js`)
- **Primary/Secondary color slots**: Clickable swatches
- **Shift+click**: Swap primary/secondary
- **D**: Reset to defaults
- **Recent colors grid**: Limited recent swatches
- **Color wheel**: Full HSV/RGB color picker
- **Hex input fields**
- **Palette management**: Default + user palettes

### 2.6 Keyboard Shortcuts (`src/ui/toolbar/shortcuts.js`)
- **Modifier handling**: Ctrl, Meta, Shift, Alt
- **Tool shortcuts** (no modifier): `b`=pen, `e`=eraser, `g`=fill, `i`=eyedropper, `l`=line, `r`=rect, `u`=rrect, `o`=ellipse, `p`=polygon, `a`=arc, `c`=chord, `v`=radial_gradient, `t`=text, `h`=hand
- **Undo/Redo**: `Ctrl+Z`/`Ctrl+Y` or `Ctrl+Shift+Z`
- **Zoom**: `+`/`-`, `0`=fit, `1`=actual
- **Pen size**: `[`/`]` (shift: -5/+5)
- **Color swap**: `x`
- **Reset colors**: `d`
- **Shortcuts are disabled** when focus is on editable elements or when UI is busy (dialog open)

---

## 3. Tool System

### 3.1 Tool Base Class (`src/tools/tool_manager.js`)
- All tools extend `Tool` base class
- Tools register with `ToolManager`
- Active tool switches with `cancel()` → `deactivate()` → `activate()` sequence
- Tools have: `id`, `cursor`, `label`, `pointerDown`, `pointerMove`, `pointerUp`, `hover`, `overlay`, `keyDown`, `cancel`

### 3.2 Pen Tool (`src/features/pen/pen.js`)
- **Modes**: `draw` or `erase`
- **Cursor**: Custom SVG cursor or eraser cursor
- **Key behaviors**:
  - `Alt+click` = eyedropper (sample pixel color)
  - `Shift+click` = straight line segment from last point
  - Commits edit on pointer up
  - Live overlay preview (semi-transparent brush footprint at zoom >= 2)
  - Pen size 1-64, configurable via option bar

### 3.3 Eraser Tool
- Shares structure with Pen Tool, mode="erase"
- Different cursor icon

### 3.4 Fill Tool (`src/features/fill/`)
- Flood-fill click operation
- Tolerance: 0 (exact match)
- Fills connected region of primary color

### 3.5 EyeDropper Tool
- `Alt+click` on canvas samples pixel
- Sets primary color to sampled color
- Info toast: "투명 픽셀" if clicking transparent pixel

### 3.6 Shape Tools (`src/features/shape/`)
- **Tools**: line, rect, rrect, ellipse, polygon
- **Option bar section**: `shape` (visible when shape tool active)
- **Controls**:
  - Fill/toggle (outline vs fill)
  - Radius (for rrect only)
  - Lock aspect ratio
  - Snap to grid/unit
  - Bounding box inputs (x, y, w, h) or polygon vertices
  - Commit (Enter) / Cancel (Esc)
- **Polygon vertex editing**: Add vertices, remove vertices, drag to adjust

### 3.7 Gradient Tool (`src/features/gradient/`)
- Radial gradient tool

### 3.8 Text Tool (`src/features/text/text_tool.js`)
- **Flow-text model**: Rasterizes text to pixel mask
- **Glyph sets**: Loaded from `./tools/recipe/text_glyphs.json`
- **Character support**: Only supported glyphs render; unsupported chars show warning
- **Input**: `pointerDown` commits entire visible canvas area with text mask
- **Cursor**: `text` cursor

### 3.9 Hand Tool
- Drag to pan canvas
- Wheel to zoom

---

## 4. Input Handling

### 4.1 Pointer Events (`src/tools/input_controller.js`)
- Manages `pointerdown`, `pointermove`, `pointerup`
- Routes to active tool's pointer methods
- Handles modifier keys (Alt, Shift)
- Maintains hover state for cursor updates

### 4.2 Keyboard Events (`src/ui/toolbar/shortcuts.js`)
- `createShortcuts()` attaches `keydown` handler
- `isEditableTarget()`: Allows Escape to blur focused inputs
- `isBusyUI()`: Disables shortcuts when dialog open or `aria-busy`
- Tool manager's `keyDown()` is tried first, then shortcut resolution

---

## 5. State Persistence & Autosave

### 5.1 Storage (`src/ui/shared/storage.js`)
- `restoreSettings()` / `storeJson()`: Persists settings to IndexedDB
- `AutosaveStore` (`src/io/store_idb.js`): Open/peek/load cycle
- On boot: Prompt to restore from auto-save or start new

### 5.2 Settings Schema (`src/core/session.js`)
```javascript
const DEFAULT_SETTINGS = Object.freeze({
  primaryColor: "#000000",
  secondaryColor: "#ffffff",
  penSize: 1,
  activeTool: "pen",
  gridMode: "off",
  snapUnit: false,
  shapeFill: "outline",
  shapeRadius: 8,
  shapeLockAspect: false,
});
```
- `validateSetting()` validates before setting
- `SETTING_KEYS` defines all savable settings

### 5.3 History/Undo (`src/features/document/session.js`)
- `UndoManager` with `UNDO_MAX_STEPS = 200`
- `UNDO_MAX_BYTES = 96MB`
- `execute(cmd)` / `record(cmd)`: Commit with applied flag
- `undo()` / `redo()`: History navigation
- Events: `HISTORY_CHANGED`, `PIXELS_CHANGED`, `LAYERS_CHANGED`

---

## 6. Rust Porting Considerations

### 6.1 Key Challenges

| Area | JS Implementation | Rust Port Considerations |
|------|-------------------|-------------------------|
| **Event System** | `EventTarget` + `CustomEvent` | Use `web_sys` `EventTarget` or custom pub/sub |
| **DOM Manipulation** | `document.createElement`, `appendChild` | Use `dioxus`, `yew`, or `rsx` macro |
| **IndexedDB Storage** | `idb` library | Use `indexeddb-futures` or `tauri-plugin-store` |
| **Canvas Rendering** | `CanvasRendering2D` | Use `dioxus-web` canvas or `leptos` with `web-sys` |
| **Pointer Events** | `pointerdown/move/up` | Map to `MouseEvent`/`TouchEvent` |
| **Keyboard Events** | `keydown` handler | Dioxus/yew `on_key_down` callbacks |
| **Modals/Dialogs** | Custom DOM construction | `yew-dialog`, `dioxus-modals`, or custom `rsx` |
| **Color Picker** | Complex DOM + wheel events | `minui`/`slint` color selectors or custom widget |
| **Undo History** | Custom `UndoManager` | State delta tracking, byte budget |

### 6.2 Recommended Rust Framework Approaches

**Option A: Dioxus + Tauri**
- Use `use_state` for reactive state
- `use_effect` for event subscriptions
- `dioxus-router` for page navigation
- Canvas via `rsx` with `web-sys` `CanvasRenderingContext2d`
- IndexedDB via `use_local_storage` or `indexeddb` crate

**Option B: Leptos + Tauri**
- Signal-based reactivity
- `use_action` for async operations
- Leptos canvas components
- Better TypeScript compatibility

**Option C: Yew**
- Minimalist approach
- `use_reducer` for state management
- Canvas via `yew-canvas`
- Good for pure WASM targets

### 6.3 UI/UX Parity Checklist

- [ ] Toolbar button icons with Korean fallbacks or SVG
- [ ] Menubar hover/click behavior with 220ms delay
- [ ] Option bar sections visible per active tool
- [ ] Pen size: number input + range slider + 15-grid preset grid (4 columns)
- [ ] Pen size wheel adjustment (Shift for 5-step, default 1-step)
- [ ] Brush footprint preview in option bar
- [ ] Status bar: zoom, canvas size, grid coords, cursor pos, tool, dirty dot
- [ ] Layer panel: thumbnails, opacity, blend modes, rename, lock, reorder, merge
- [ ] Grid mode cycle: off → unit → tile → pixel (Ctrl+')
- [ ] Zoom levels: [25%, 50%, 100%, 200%, 300%, 400%, 600%, 800%, 1200%, 1600%, 2400%, 3200%]
- [ ] Fit to window, Actual size (100%)
- [ ] Menubar file/edit/view/layer menus
- [ ] Undo/redo (Ctrl+Z/Y) with disabled state when at limits
- [ ] Keyboard shortcuts matching JS version exactly
- [ ] Color primary/secondary slots with Shift swap
- [ ] Recent colors grid
- [ ] Color wheel picker
- [ ] New document dialog: tile inputs, presets, background color, name
- [ ] Resize canvas dialog with 32px unit constraint
- [ ] Confirm discard changes modal
- [ ] Auto-save restore dialog
- [ ] About/status toasts at top of screen
- [ ] Keyboard focus management in modals (Tab cycle, Escape close)
- [ ] ARIA states (`aria-pressed`, `aria-expanded`, `aria-label`)
- [ ] Cursor changes per tool (pen, eraser, text, hand)

---

## 7. Detailed Interaction Patterns

### 7.1 Tool Switching
When active tool changes:
1. Old tool: `deactivate()` → cleanup
2. New tool: `activate()` → initialize
3. Toolbar buttons: `aria-pressed` sync via `SETTINGS_CHANGED` event
4. Option bar sections: visibility toggle based on `activeTool`
5. Status bar: tool label update

### 7.2 Pen Size Adjustment
Multiple pathways:
1. **Option bar number input**: Type value, change event → `setSetting("penSize", n)`
2. **Option bar range slider**: Input change → update pen size
3. **Option bar preset grid**: Button click → set specific size
4. **Wheel on number/range input**: `wheel` event → `stepPenSize(current, deltaY, shiftKey)`
5. **Keyboard `]/[`**: `Ctrl+shift` → ±5, otherwise ±1
6. **Status bar not directly involved**, but `sync()` propagates changes

### 7.3 Color Selection
1. **Color swatch click**: Sets primary or secondary color
2. **Shift+click**: Swaps primary/secondary
3. **Color wheel**: Full HSV picker, updates both slots
4. **Recent colors grid**: Click → sets primary color
5. **Hex input**: Type `#rrggbb` or `#rrggbbaa`, commit
6. **Reset (D)**: Sets primary=black, secondary=white

### 7.4 Layer Operations
- **Add**: New layer at top of stack
- **Duplicate**: Copy active layer
- **Remove**: Delete active layer (disabled if only 1 layer)
- **Move Up/Down**: Reorder in stack
- **Merge Down**: Merge active layer into below layer
- **Opacity**: Slider (0-100%) with number input
- **Blend Mode**: Dropdown with 6 modes
- **Visibility (eye)**: Show/hide layer
- **Lock/unlock**: Lock icon, prevents editing
- **Rename**: Double-click name, type new name, Enter to confirm, Escape to cancel

### 7.5 Canvas Interaction
- **Pointer enter**: Cursor changes to active tool cursor
- **Pointer leave**: Status bar shows `- , -` off-canvas position
- **Zoom**: Wheel over canvas, or toolbar buttons, or status bar fit/actual
- **Pan**: Hand tool drag, or middle-drag anywhere
- **Coordinate readout**: Status bar pixel + grid/tile readout follows pointer

### 7.6 Undo/Redo State
- Enabled/disabled based on history capabilities
- Menubar buttons: `disabled = busy || !canUndo/!canRedo`
- Keyboard shortcuts: silently no-op when at limits
- `HISTORY_CHANGED` event drives UI state

---

## 8. Accessibility Summary

- All interactive elements have `aria-label`, `aria-pressed`, or `aria-expanded`
- Focus management in modals (Trapped focus, Tab cycle)
- `Escape` closes modals and dialogs
- Color contrast: Minimum viable for pixel art tool
- Alternative text for icons (Korean character fallbacks)
- Button text content is meaningful (not just icons without labels)

---

## 9. Rust Port Priority Matrix

### Phase 1 (Core Functionality)
1. Canvas rendering and pointer events
2. Basic tools: pen, eraser, fill
3. Undo/redo system
4. Settings persistence

### Phase 2 (UI Panels)
5. Toolbar with icons/fallbacks
6. Status bar
7. Option bar (pen size, basic controls)

### Phase 3 (Advanced Features)
8. Shape tools with vertex editing
9. Layer panel with thumbnails and operations
10. Color panel with wheel and recent colors

### Phase 4 (Polish)
11. Keyboard shortcuts (full mapping)
12. Menubar with hover behavior
13. Dialog system (new doc, resize, confirm)
14. Toasts and status messages

---
*End of Report*