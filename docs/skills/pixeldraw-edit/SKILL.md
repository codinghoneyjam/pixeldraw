---
name: pixeldraw-edit
description: Make precise, reproducible pixel and shape edits to Pixeldraw documents with the Rust CLI.
---

# Pixeldraw structured editing

Use the bundled `pixeldraw-cli` binary to inspect and make pixel-precise edits to `.draw.json` documents. The CLI and GUI use the same Rust shape rasterizer and document serializer.

## Workflow

1. Inspect the source with `pixeldraw-cli info <source.draw.json>` and `pixeldraw-cli layers <source.draw.json>`. Resolve stable layer IDs before editing; exact names are accepted only when unique.
2. Create a JSON patch file with an `operations` array. Use integer pixel coordinates and RGBA byte arrays. Prefer a stable ID for every target layer.
3. Run `pixeldraw-cli edit <source.draw.json> <patch.json> <result.draw.json>`. The command applies the whole patch to a private copy and writes only after every operation and document validation pass. Review `pixel_changes` in the JSON response for exact changed pixel counts and bounds.
4. Inspect `result.draw.json` with `pixeldraw-cli info`, render it with `pixeldraw-cli flatten <result.draw.json> <preview.png>`, and open the result in the GUI for visual approval.
5. Keep the original source until the result is accepted. The CLI is a batch workflow and does not add entries to the GUI undo history.

See `docs/cli_edit.md` for operation schemas and examples. Supported operations are `set_pixel`, `draw_shape`, `set_layer`, `move_layer`, and `set_atlas`. Shapes include line, rectangle, rounded rectangle, ellipse, and polygon. Unknown patch keys, ambiguous layer names, invalid integer coordinates, out-of-canvas pixels, locked-layer writes, and invalid rasterizer parameters fail with an indexed JSON error.

## Atlas workflow

For sprite sheets, inspect `pixeldraw-cli atlas spec`, plan with `pixeldraw-cli atlas plan <doc> [options]`, and export with `pixeldraw-cli atlas export <doc> <out.png> [options]`. The shared engine supports up to 64 cells, subject to the 64 Mi-pixel sheet budget. Use the GUI for interactive cell selection, slot composition, duplication, import preview, and editing a source layer from a cell.

See `docs/skills/pixeldraw-atlas/SKILL.md` for atlas options and metadata behavior.
