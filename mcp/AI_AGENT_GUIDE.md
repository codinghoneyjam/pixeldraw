# PixelDraw MCP & Tool Integration Guide for AI Agents

> **For AI Assistants (Claude, Cursor, Copilot, Gemini, GPT)**  
> This document specifies the `draw_image` tool interface and its exact query DSL syntax. When you need to generate pixel-art images, sprites, tiles, or icons for this project, construct queries conforming to the rules below and call `draw_image`.

---

## 1. Tool Interface Specification

### `draw_image`
Executes PixelDraw Query DSL commands, rasters them through pure-ESM pixel pipeline, and saves the resulting image as a PNG directly to the specified file path.

#### Arguments:
- **`query`** (`string`, **required**): One or more DSL drawing commands separated by newlines (`\n`).
- **`output_path`** (`string`, **required**): Absolute or relative file path to save the `.png` file (e.g., `query_result/avatar.png`). Missing directories are automatically created.
- **`width`** (`number`, *optional*): Canvas width in pixels (default: `64`).
- **`height`** (`number`, *optional*): Canvas height in pixels (default: `64`).
- **`background`** (`string`, *optional*): Canvas background color in hex (`#FFFFFF`) or `"transparent"` (default: `"transparent"`).

---

## 2. DSL Syntax Reference (Match to Internal Functions)

Coordinate System: `(0, 0)` is the **top-left** corner. All dimensions are in integer pixels.

### 2.1 Canvas Setup
```dsl
canvas w <width> h <height> [bg <hex|transparent>]
```
- Example: `canvas w 64 h 64 bg transparent`

### 2.2 Circle & Ellipse (`applyShape: ellipse`)
```dsl
circle x <x> y <y> w <width> h <height> [color <hex>] [fill <hex>] [outline <hex>] [width <stroke_px>]
```
- `color` or `fill`: Fills the shape with the color.
- `outline`: Draws an outer border ring with the color.
- `width`: Border line stroke thickness (default: 1).
- *Alternative center/radius format*:
  ```dsl
  circle cx <center_x> cy <center_y> r <radius> color <hex>
  ```
- Examples:
  - `circle x 3 y 4 w 24 h 38 color #77832F`
  - `circle x 10 y 10 w 44 h 44 fill #3B82F6 outline #1D4ED8 width 2`

### 2.3 Rectangle & Rounded Rectangle (`applyShape: rrect`)
```dsl
rect x <x> y <y> w <width> h <height> [radius <r>] [color <hex>] [fill <hex>] [outline <hex>] [width <stroke_px>]
```
- `radius`: Corner rounding radius in pixels (default: 0 for sharp rectangle).
- Examples:
  - `rect x 0 y 0 w 64 h 64 fill #1E293B`
  - `rect x 8 y 8 w 48 h 48 radius 6 fill #10B981 outline #047857 width 2`

### 2.4 Straight Line (`applyShape: line`)
```dsl
line x0 <x0> y0 <y0> x1 <x1> y1 <y1> color <hex> [width <stroke_px>]
```
- Example: `line x0 0 y0 0 x1 63 y1 63 color #FFFFFF width 2`

### 2.5 Polygon (`applyShape: polygon`)
```dsl
polygon pts <x1,y1> <x2,y2> <x3,y3> ... [color <hex>] [fill <hex>] [outline <hex>] [width <stroke_px>]
```
- Example (Triangle): `polygon pts 32,8 56,52 8,52 fill #F59E0B outline #B45309 width 1`

### 2.6 Single Pixel (`writer.set`)
```dsl
pixel x <x> y <y> color <hex>
```
- Example: `pixel x 32 y 32 color #EF4444`

---

## 3. Strict Rules for AI Generation

1. **Layer Order**: Commands are drawn sequentially from top to bottom. Backgrounds or base shapes MUST come first; highlights, details, and outlines MUST follow.
2. **Color Format**: Always use standard Hex notation:
   - `#RGB` (e.g. `#FFF`)
   - `#RRGGBB` (e.g. `#77832F`)
   - `#RRGGBBAA` (e.g. `#77832F80` for 50% opacity)
   - Or `"transparent"`
3. **Bounding Box**: Ensure `w` and `h` are positive integers `>= 1`.
4. **Target Path**: Ensure `output_path` ends with `.png`.

---

## 4. Full Working Examples

### Example A: Potion Bottle Icon
```dsl
canvas w 32 h 32 bg transparent
# Bottle neck
rect x 13 y 4 w 6 h 6 fill #E2E8F0 outline #475569 width 1
# Cork
rect x 14 y 2 w 4 h 3 fill #92400E
# Bottle body
circle x 6 y 9 w 20 h 20 fill #818CF8 outline #3730A3 width 1
# Liquid highlight
circle x 9 y 12 w 5 h 5 fill #C7D2FE
```

### Example B: Game Shield Sprite
```dsl
canvas w 48 h 48 bg transparent
polygon pts 24,4 42,12 38,36 24,44 10,36 6,12 fill #3B82F6 outline #1E3A8A width 2
line x0 24 y0 6 x1 24 y1 42 color #FDE047 width 2
line x0 10 y0 22 x1 38 y1 22 color #FDE047 width 2
```
