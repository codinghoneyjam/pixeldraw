# <META - FILE SUMMARY - Split io/serialize.js along its raster / vector / recipe seams>
"""
serialize.js carries three unrelated responsibilities behind one 500+ line file:

  * raster document/layer JSON  (schema write, validate, chunk PNG decode)
  * vector JSON                 (chunks -> commands, commands -> Document)
  * legacy recipe JSON          (surface / pantograph / atlas / standard import)

This script cuts the file at those seams, prunes each half's imports to what the
moved code actually uses, and keeps `serialize.js` as a re-export barrel so no
caller (`src/ui/actions/file_actions.js`, tests, `tools/recipe/*`) has to change.

Re-run safety: the script asserts the exact cut points first, so a drifted
serialize.js aborts before anything is overwritten. It also refuses to run twice
(the output files must not already exist).
"""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
P = ROOT / "src" / "io" / "serialize.js"
VECTOR_OUT = ROOT / "src" / "io" / "serialize_vector.js"
RECIPE_OUT = ROOT / "src" / "io" / "serialize_recipe.js"


def main() -> int:
    lines = P.read_text(encoding="utf-8").split("\n")
    print(f"serialize.js: {len(lines)} lines")

    # --- cut-point assertions (0-based indices; 1-based line in the comment) ---
    assert lines[256] == "}", repr(lines[256])            # 257 end importLayerJson
    assert lines[257] == "", repr(lines[257])             # 258 blank separator
    assert lines[258].startswith("// <META - ROLE : Extract vector"), repr(lines[258])
    assert lines[407] == "}", repr(lines[407])            # 408 end vectorJsonToDocument
    assert lines[408] == "", repr(lines[408])             # 409 blank separator
    assert lines[409].startswith("// <META - ROLE : Shift one point"), repr(lines[409])
    assert lines[571] == "}", repr(lines[571])            # 572 EOF
    print("cut points verified")

    for out in (VECTOR_OUT, RECIPE_OUT):
        if out.exists():
            print(f"ABORT: {out.name} already exists — delete it first if you meant to re-run")
            return 1

    raster = lines[0:257]   # lines 1-257   raster document/layer JSON
    vector = lines[258:408] # lines 259-408 vector JSON
    recipe = lines[409:572] # lines 410-572 legacy recipe import

    vector_header = '''// <META - FILE SUMMARY - Vector JSON: trace raster chunks to commands + bake commands back>
// Split out of serialize.js: the vector half of import/export. serialize.js keeps
// the raster document/layer JSON half and re-exports these four entry points.
// <META - SUMMARY CONT - documentToVectorJson scans every non-empty chunk into
// <META - SUMMARY CONT - rect/pixel commands; vectorJsonToDocument replays them
// <META - SUMMARY CONT - through applyCommand, resolving "$name" palette tokens.
import { SCHEMA_VERSION, CHUNK_PX } from "../core/constants.js";
import { ChunkStore, PixelWriter } from "../core/chunkstore.js";
import { DrawToolError } from "../core/errors.js";
import { Document, IdGen } from "../features/document/document.js";
import { Layer } from "../features/layers/layer.js";
import { applyCommand } from "../../tools/recipe/render_tile.js";
'''.split("\n")

    recipe_header = '''// <META - FILE SUMMARY - Legacy recipe JSON import: bake surface/pantograph/atlas/standard recipes>
// Split out of serialize.js: recipeJsonToDocument mirrors the transpile branches
// in-browser so a recipe file imports identically to the Node-side transpiler.
// <META - SUMMARY CONT - Routing order is surface manifest -> pantograph profile ->
// <META - SUMMARY CONT - atlas slots -> standard recipe; shiftCommand keeps
// <META - SUMMARY CONT - per-slot commands laid out left to right on one sheet.
import { ChunkStore, PixelWriter } from "../core/chunkstore.js";
import { packRGBA } from "../core/pixel.js";
import { DrawToolError } from "../core/errors.js";
import { Document, IdGen } from "../features/document/document.js";
import { Layer } from "../features/layers/layer.js";
import { applyCommand, renderTile } from "../../tools/recipe/render_tile.js";
import { assembleSheet } from "../core/raster/atlas.js";
import { isPantographProfile, isSurfaceManifest, pantographCommandsForLayers, surfaceCommandsFor } from "../../tools/recipe/schema.mjs";
'''.split("\n")

    VECTOR_OUT.write_text("\n".join(vector_header + vector) + "\n", encoding="utf-8")
    RECIPE_OUT.write_text("\n".join(recipe_header + recipe) + "\n", encoding="utf-8")
    print(f"wrote serialize_vector.js  ({VECTOR_OUT.read_text(encoding='utf-8').count(chr(10))} lines)")
    print(f"wrote serialize_recipe.js ({RECIPE_OUT.read_text(encoding='utf-8').count(chr(10))} lines)")

    # --- prune serialize.js imports down to what the raster half still uses ---
    drop = [
        'import { ChunkStore, PixelWriter } from "../core/chunkstore.js";',
        'import { packRGBA } from "../core/pixel.js";',
        'import { applyCommand, renderTile } from "../../tools/recipe/render_tile.js";',
        'import { assembleSheet } from "../core/raster/atlas.js";',
        'import { isPantographProfile, isSurfaceManifest, pantographCommandsForLayers, surfaceCommandsFor } from "../../tools/recipe/schema.mjs";',
    ]
    add = [
        'import { ChunkStore, isAllZero } from "../core/chunkstore.js";',
    ]
    # Barrel re-exports keep io/serialize.js the single import site for io callers.
    barrel = [
        "",
        "// Barrel: the vector and recipe halves of import/export live in sibling modules.",
        "// Re-exported so src/ui/actions/file_actions.js and tools/recipe/* keep one import site.",
        'export { documentToVectorJson, vectorJsonToDocument } from "./serialize_vector.js";',
        'export { recipeJsonToDocument, shiftCommand } from "./serialize_recipe.js";',
    ]

    out: list[str] = []
    dropped_chunkstore = False
    for ln in raster:
        if ln in drop:
            if not dropped_chunkstore and ln.startswith('import { ChunkStore'):
                out.extend(add)
                dropped_chunkstore = True
            continue
        out.append(ln)
    if not dropped_chunkstore:
        print("ABORT: expected to drop the ChunkStore/PixelWriter import, pattern drifted")
        return 1

    # The old `isAllZero` import line is superseded by the merged one above.
    out = [ln for ln in out if ln != 'import { isAllZero } from "../core/chunkstore.js";']

    # Insert the barrel right after the last import line.
    last_import = max(i for i, ln in enumerate(out) if ln.startswith("import "))
    out = out[: last_import + 1] + barrel + out[last_import + 1 :]

    P.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"rewrote serialize.js       ({P.read_text(encoding='utf-8').count(chr(10))} lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
