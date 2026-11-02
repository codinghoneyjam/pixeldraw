# Structured document edits from CLI

`pixeldraw-cli edit` applies integer-coordinate edits to a saved Pixeldraw JSON document. It is intended for scripts and AI agents that need reproducible pixel-level changes while preserving layers and atlas layout.

```text
pixeldraw-cli edit input.draw.json patch.json output.draw.json
```

The patch is one JSON object with an `operations` array. All operations are applied to a private copy. If an operation fails, the command reports its array index, returns a non-zero exit code, and does not write the output document. The result is validated against the document schema before writing. Output JSON can be opened in the GUI and keeps the existing document and atlas fields. The JSON response includes `pixel_changes` per layer: changed pixel count and the smallest enclosing `[x,y,width,height]` bounds as an object.

Layer selectors accept a stable layer ID or an exact layer name. Duplicate names are ambiguous and require an ID. Layer order indexes are zero-based, bottom-to-top. Colors are `[red, green, blue, alpha]` channels from 0 to 255. Coordinates and shape dimensions are integer pixels.

## Set one pixel

```json
{
  "operations": [
    { "op": "set_pixel", "layer": "layer-0001", "x": 12, "y": 9, "rgba": [240, 80, 40, 255] }
  ]
}
```

`set_pixel` rejects coordinates outside the canvas and refuses to write to a locked layer. Alpha zero clears the pixel.

## Draw a shape

```json
{
  "operations": [
    {
      "op": "draw_shape",
      "layer": "Body",
      "kind": "line",
      "p0": [8, 14],
      "p1": [22, 9],
      "stroke_width": 1,
      "primary": [32, 24, 40, 255]
    },
    {
      "op": "draw_shape",
      "layer": "Body",
      "kind": "ellipse",
      "bbox": [10, 10, 16, 12],
      "fill": "both",
      "primary": [32, 24, 40, 255],
      "secondary": [250, 190, 90, 255]
    }
  ]
}
```

Supported `kind` values are `line`, `rect`, `rrect`, `ellipse`, and `polygon`. Boxed shapes use `[x,y,width,height]` in `bbox`; lines use `p0` and `p1`; polygons use `points` as `[[x,y], ...]`. `fill` is `outline` (default), `fill`, or `both`. In `both` mode, `secondary` fills the shape and `primary` draws its outline. `stroke_width` defaults to one pixel and follows the engine's 1–64 pixel limit. Rounded rectangles accept `radius`.

## Layer metadata and order

```json
{
  "operations": [
    { "op": "set_layer", "layer": "layer-0001", "name": "Body clean-up", "visible": true, "locked": false, "opacity": 1.0 },
    { "op": "move_layer", "layer": "Body clean-up", "to_index": 2 }
  ]
}
```

`set_layer` accepts any non-empty combination of `name`, `visible`, `locked`, and `opacity`. Opacity is in `[0,1]`. `move_layer.to_index` is zero-based in bottom-to-top order.

## Set or clear the saved atlas layout

`set_atlas` stores the same layout object used by the GUI and atlas PNG metadata. Set `layout` to a complete `AtlasLayout` object; set it to `null` to remove the saved layout.

```json
{
  "operations": [
    {
      "op": "set_atlas",
      "layout": {
        "mode": "composed",
        "columns": 2,
        "rows": 1,
        "cell_w": 64,
        "cell_h": 64,
        "margin": 4,
        "padding": 8,
        "preset_all": false,
        "slots": [
          { "name": "Idle", "layer_ids": ["base", "eyes"], "merged": true },
          { "name": "Run", "layer_ids": ["base", "legs-run"], "merged": true }
        ]
      }
    }
  ]
}
```

The CLI validates grid capacity, sheet budget, slot layer IDs, and the per-slot layer limit before writing. Saving the resulting `.draw.json` preserves the layout for the GUI and future atlas exports.

## Limits

- At most 10,000 operations per patch.
- All shape rasterization uses the same Rust engine as the GUI.
- The `pixel_changes` response is an exact pixel diff summary. Use `flatten` on the result for a visual preview.
- Supported operations: `set_pixel`, `draw_shape`, `set_layer`, `move_layer`, and `set_atlas`.
- This is a batch file edit: the CLI has no interactive preview or GUI undo history. Keep the source file and inspect the output in Pixeldraw before replacing a production asset.
- A PNG import remains raster data; geometry baked into PNG cells cannot become editable vector objects through this patch format.
