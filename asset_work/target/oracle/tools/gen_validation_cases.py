# <META - FILE SUMMARY - Project module implementation>
"""Writes tests/fixtures/validation_cases.json: documents + expected verdicts for the JS validator/importer.
Oracle = schema/schema_selfcheck.py (JSON Schema + semantic rules). Run from anywhere."""
import copy, json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "schema"))
import schema_selfcheck as sc  # noqa: E402

sample = sc.build_sample()
raster_only = copy.deepcopy(sample)
raster_only["layers"] = [l for l in raster_only["layers"] if l["type"] == "raster"]

# <META - ROLE : Execute m | L16-17>
def m(fn, base=sample):
    d = copy.deepcopy(base); fn(d); return d

cases = []
# <META - ROLE : Execute add | L21-25>
def add(name, doc, valid, code=None, import_error=None):
    verdict = (not list(sc.DOC_V.iter_errors(doc))) and not sc.semantic_errors(doc)
    assert verdict == valid, "oracle disagrees with declared verdict: " + name
    cases.append({"name": name, "expect_valid": valid, "expect_code": code,
                  "expect_import_error": import_error, "document": doc})

add("raster_only_sample", raster_only, True)
add("sample_with_vector_layer", sample, True, import_error="UNSUPPORTED_LAYER_TYPE")
schema_bad = {
 "width_not_multiple_of_32": lambda d: d["canvas"].__setitem__("width_px", 100),
 "width_above_1920": lambda d: d["canvas"].__setitem__("width_px", 1952),
 "height_above_1088": lambda d: d["canvas"].__setitem__("height_px", 1120),
 "tile_px_changed": lambda d: d["canvas"].__setitem__("tile_px", 32),
 "unknown_blend": lambda d: d["layers"][0].__setitem__("blend", "dissolve"),
 "opacity_above_1": lambda d: d["layers"][0].__setitem__("opacity", 1.5),
 "raster_layer_with_shapes": lambda d: d["layers"][0].__setitem__("shapes", []),
 "chunk_png_signature_missing": lambda d: d["layers"][0]["raster"]["chunks"][0].__setitem__("png", "AAAA"),
 "cx_above_59": lambda d: d["layers"][0]["raster"]["chunks"][0].__setitem__("cx", 60),
 "cy_above_33": lambda d: d["layers"][0]["raster"]["chunks"][0].__setitem__("cy", 34),
 "extra_root_property": lambda d: d.__setitem__("order", 1),
 "empty_layers": lambda d: d.__setitem__("layers", []),
 "wrong_schema_version": lambda d: d.__setitem__("schema_version", "1.0.0"),
 "missing_format": lambda d: d.pop("format"),
}
for k, fn in schema_bad.items():
    add(k, m(fn, raster_only), False, "SCHEMA")
first = raster_only["layers"][0]["raster"]["chunks"][0]["png"]
sem = {
 "duplicate_chunk": ("CHUNK_DUPLICATE", lambda d: d["layers"][0]["raster"]["chunks"].append(dict(d["layers"][0]["raster"]["chunks"][0]))),
 "chunk_outside_canvas": ("CHUNK_OUT_OF_CANVAS", lambda d: d["layers"][0]["raster"]["chunks"].append({"cx": 5, "cy": 0, "png": first})),
 "chunk_wrong_size": ("CHUNK_BAD_PNG", lambda d: d["layers"][0]["raster"]["chunks"][0].__setitem__("png", sc.png_b64(sc.Image.new("RGBA", (16, 16), (1, 2, 3, 255))))),
 "duplicate_layer_id": ("DUP_LAYER_ID", lambda d: d["layers"][1].__setitem__("layer_id", "layer-0001")),
 "dangling_active_layer": ("ACTIVE_LAYER_MISSING", lambda d: d.__setitem__("active_layer_id", "nope")),
}
for k, (code, fn) in sem.items():
    add(k, m(fn, raster_only), False, code)

out = ROOT / "tests" / "fixtures" / "validation_cases.json"
out.write_text(json.dumps({"cases": cases}, indent=1) + "\n", encoding="utf-8")
(ROOT / "tests" / "fixtures" / "sample_raster_only.json").write_text(json.dumps(raster_only, indent=1) + "\n", encoding="utf-8")
print("wrote", len(cases), "validation cases")
