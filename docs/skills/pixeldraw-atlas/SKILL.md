---
name: pixeldraw-atlas
description: Build and inspect Pixeldraw atlases headlessly with pixeldraw-cli. Use when an agent must list document layers, plan a 1×16…16×1 atlas grid, preview ordered layer combinations, export a layer-per-cell or composed-slot sheet, or read `dtAt` metadata back.
---

# Pixeldraw atlas skill

`pixeldraw-cli` is the headless half of the atlas workbench. It uses the same
`pixel-engine` rasteriser as the editor, so a sheet it writes is byte-for-byte
the sheet the GUI would write for the same layout.

Every invocation prints **one JSON object** and exits non-zero on failure. Do
not scrape human text; read `ok`, `error.code`, `error.message`, `sheet`,
`cell`, `cells`, and `metadata`.

## Build

```sh
cargo build -p pixeldraw-cli --release
# target/release/pixeldraw-cli
```

## Discover the limits

```sh
pixeldraw-cli atlas spec
```

Returns `max_cells` (16), `max_layers_per_cell` (16), `default_cell_max`
(512), `max_pixels` (64 Mi px), `metadata_version` (2), and the save modes.

## Inspect a document

```sh
pixeldraw-cli info assets/my.draw.json
pixeldraw-cli layers assets/my.draw.json
```

`layers` is returned **top-first**, matching the layers panel. `layer_ids` in
atlas options and metadata are **bottom → top**, matching the compositor.

## Plan before writing

```sh
pixeldraw-cli atlas plan assets/my.draw.json \
  --mode composed \
  --columns 4 \
  --cell 256 \
  --slot "Idle=Body,Outline" \
  --slot "Hit=Body,Outline,Flash"
```

The plan contains no pixels; it returns the sheet size, cell size, grid, and
the exact `metadata` that `atlas export` will embed.

Options:

| Option | Meaning |
| --- | --- |
| `--mode layers\|composed` | Default is `composed` when `--slot` is used, otherwise `layers`. |
| `--columns N` | 1…16. Rows are `ceil(cells / columns)`; all layouts from 1×16 to 16×1 are reachable. |
| `--cell N` or `--cell WxH` | Cell size. Default clamps the document canvas to 512×512. |
| `--original-size` | Use the document canvas size as the cell size. |
| `--no-preset` | Do not prepend the automatic “all layers” slot. |
| `--slot NAME=a,b,c` | Ordered slot, **bottom → top**. Repeatable; up to 16 slots/layers. |

The preset slot is on by default: in composed mode cell 1 is every layer in
document order. `--no-preset` writes only the user slots.

## Save modes

### 1. Layer-per-cell (“as-is” sheet)

One cell per document layer, top-first:

```sh
pixeldraw-cli atlas export assets/my.draw.json out/atlas_layers.png \
  --mode layers --columns 4
```

Each cell contains that layer at full document resolution, resampled
aspect-fit to the cell. This is the “giant canvas with each layer in its own
cell” save.

### 2. Composed slots

One cell per ordered combination:

```sh
pixeldraw-cli atlas export assets/my.draw.json out/atlas_slots.png \
  --mode composed --columns 4 --cell 256 \
  --slot "Walk=Body,Legs" \
  --slot "Walk+Flash=Body,Legs,Flash"
```

Layers in `--slot` are applied in the order given (last one on top). The
preset all-layers cell is first unless `--no-preset` is passed.

## Read an atlas back

```sh
pixeldraw-cli atlas inspect out/atlas_slots.png
```

Returns the PNG size plus the `dtAt` metadata: version, mode, grid, cell,
sheet, preset flag, slot names, source layer names, and layer ids. v1 payloads
from older builds still parse as `mode = "layers"` with no `layer_ids`.

In the GUI, **PNG에서 가져오기** (or File → Open) reconstructs a **cell-sized**
document with one layer per sheet slot, so the round trip returns to the same
editing unit instead of a sheet-sized canvas.

## One cell without a sheet

```sh
pixeldraw-cli atlas render-cell assets/my.draw.json out/cell.png \
  --cell 256 --layers "Body,Outline"
```

Useful for thumbnails or for feeding an image model a single composed sprite.

## Flatten a normal document

```sh
pixeldraw-cli flatten assets/my.draw.json out/flat.png
```

## Failure contract

```json
{
  "ok": false,
  "error": { "code": "OUT_OF_RANGE", "message": "atlas sheet ... exceeds ..." }
}
```

Common codes: `LAYER_NOT_FOUND` (bad selector), `INVALID_STATE` (unknown
option, empty layout, bad JSON), `OUT_OF_RANGE` (grid/cell/pixel budget).
