# <META - FILE SUMMARY - Project module implementation>
"""Draw-tool schema v2 self-check (limits: 2048x1088): positive/negative validation, semantic rules,
v1->v2 migration, flatten parity, and a synthetic size benchmark.

Run: python schema_selfcheck.py            (contract checks, writes sample_document.json)
     python schema_selfcheck.py --bench    (1920x1088 size/time numbers, synthetic data)
Deterministic: no unseeded randomness.
"""
from __future__ import annotations

import base64, copy, io, json, random, sys, time
from pathlib import Path

from jsonschema import Draft202012Validator
from PIL import Image
from referencing import Registry, Resource

HERE = Path(__file__).resolve().parent
CHUNK = 32

# <META - ROLE : Execute load | L22-23>
def _load(name: str) -> dict:
    return json.loads((HERE / name).read_text(encoding="utf-8"))

_LAYER = _load("layer_schema.v2.json")
_DOC = _load("document_schema.v2.json")
_REG = Registry().with_resources([
    (_LAYER["$id"], Resource.from_contents(_LAYER)),
    (_DOC["$id"], Resource.from_contents(_DOC)),
])
DOC_V = Draft202012Validator(_DOC, registry=_REG)
LAYER_V = Draft202012Validator(_LAYER, registry=_REG)

# <META - ROLE : Execute png b64 | L35-38>
def png_b64(img: Image.Image) -> str:
    buf = io.BytesIO()
    img.save(buf, format="PNG", optimize=False)
    return base64.b64encode(buf.getvalue()).decode("ascii")

# <META - ROLE : Execute decode chunk | L41-42>
def decode_chunk(b64: str) -> Image.Image:
    return Image.open(io.BytesIO(base64.b64decode(b64))).convert("RGBA")

# <META - ROLE : Execute semantic errors | L45-72>
def semantic_errors(doc: dict) -> list[str]:
    """Rules JSON Schema cannot express."""
    errs: list[str] = []
    canvas = doc["canvas"]
    ids = [layer["layer_id"] for layer in doc["layers"]]
    if len(ids) != len(set(ids)):
        errs.append("duplicate layer_id")
    if "active_layer_id" in doc and doc["active_layer_id"] not in ids:
        errs.append("active_layer_id not found")
    shape_ids: list[str] = []
    for layer in doc["layers"]:
        seen: set[tuple[int, int]] = set()
        for ch in layer.get("raster", {}).get("chunks", []):
            key = (ch["cx"], ch["cy"])
            if key in seen:
                errs.append("duplicate chunk %s in %s" % (key, layer["layer_id"]))
            seen.add(key)
            if ch["cx"] * CHUNK >= canvas["width_px"] or ch["cy"] * CHUNK >= canvas["height_px"]:
                errs.append("chunk %s outside canvas in %s" % (key, layer["layer_id"]))
            try:
                if decode_chunk(ch["png"]).size != (CHUNK, CHUNK):
                    errs.append("chunk %s is not 32x32" % (key,))
            except Exception:
                errs.append("chunk %s PNG undecodable" % (key,))
        shape_ids += [s["shape_id"] for s in layer.get("shapes", [])]
    if len(shape_ids) != len(set(shape_ids)):
        errs.append("duplicate shape_id")
    return errs

# <META - ROLE : Execute flatten | L75-89>
def flatten(doc: dict) -> Image.Image:
    """Reference compositor: raster layers only, 'normal' blend + opacity."""
    c = doc["canvas"]
    out = Image.new("RGBA", (c["width_px"], c["height_px"]), (0, 0, 0, 0))
    for layer in doc["layers"]:
        if not layer["visible"] or layer["type"] != "raster":
            continue
        plane = Image.new("RGBA", out.size, (0, 0, 0, 0))
        for ch in layer["raster"]["chunks"]:
            plane.paste(decode_chunk(ch["png"]), (ch["cx"] * CHUNK, ch["cy"] * CHUNK))
        if layer["opacity"] < 1.0:
            alpha = plane.getchannel("A").point(lambda a: int(a * layer["opacity"]))
            plane.putalpha(alpha)
        out = Image.alpha_composite(out, plane)
    return out

# <META - ROLE : Execute migrate v1 layer | L92-101>
def migrate_v1_layer(v1: dict) -> dict:
    """v1 layer payload -> v2 vector layer (order dropped, stroke_width=2 as v1 rendered)."""
    shapes = []
    for s in v1["shapes"]:
        n = dict(s)
        n["stroke_width"] = 2
        shapes.append(n)
    return {"layer_id": v1["layer_id"], "name": v1["layer_id"], "type": "vector",
            "visible": v1["visible"], "locked": v1["locked"], "opacity": 1.0,
            "blend": v1["blend"], "shapes": shapes}

# <META - ROLE : Execute solid | L104-105>
def _solid(rgba: tuple[int, int, int, int]) -> Image.Image:
    return Image.new("RGBA", (CHUNK, CHUNK), rgba)

# <META - ROLE : Execute build sample | L108-136>
def build_sample() -> dict:
    v1 = {"schema_version": "1.0.0", "layer_id": "legacy-shapes", "order": 2, "visible": True,
          "locked": False, "blend": "normal", "shapes": [
              {"shape_id": "shape-000001", "kind": "line", "points": [[8.0, 8.0], [150.0, 90.0]],
               "fill": "#4cc2ff", "stroke": "#123a52", "corner_radius": 0.0},
              {"shape_id": "shape-000002", "kind": "bezier",
               "points": [[5.0, 5.0], [30.0, 80.0], [60.0, 10.0], [90.0, 90.0]],
               "fill": "none", "stroke": "#ff8800", "corner_radius": 0.0}]}
    line = Image.new("RGBA", (CHUNK, CHUNK), (0, 0, 0, 0))
    for i in range(CHUNK):
        line.putpixel((i, i), (18, 58, 82, 255))
    return {
        "schema_version": "2.0.0", "format": "draw_tool.document", "document_id": "sample-2x1_5-tiles",
        "name": "sample (2.5 x 1.5 tiles)",
        "canvas": {"tile_px": 64, "unit_px": 32, "width_px": 160, "height_px": 96, "background": "transparent"},
        "active_layer_id": "layer-0002",
        "layers": [
            {"layer_id": "layer-0001", "name": "base", "type": "raster", "visible": True, "locked": False,
             "opacity": 1.0, "blend": "normal",
             "raster": {"chunk_px": 32, "encoding": "png_base64", "chunks": [
                 {"cx": 0, "cy": 0, "png": png_b64(_solid((76, 194, 255, 255)))},
                 {"cx": 1, "cy": 0, "png": png_b64(_solid((76, 194, 255, 255)))},
                 {"cx": 4, "cy": 2, "png": png_b64(_solid((255, 136, 0, 255)))}]}},
            {"layer_id": "layer-0002", "name": "line art", "type": "raster", "visible": True, "locked": False,
             "opacity": 0.8, "blend": "normal",
             "raster": {"chunk_px": 32, "encoding": "png_base64", "chunks": [
                 {"cx": 0, "cy": 0, "png": png_b64(line)}, {"cx": 1, "cy": 1, "png": png_b64(line)}]}},
            migrate_v1_layer(v1)],
    }

# <META - ROLE : Execute expect invalid | L139-141>
def _expect_invalid(label: str, doc: dict, validator: Draft202012Validator = DOC_V) -> None:
    if not list(validator.iter_errors(doc)):
        raise AssertionError("accepted invalid case: " + label)

# <META - ROLE : Execute run checks | L144-205>
def run_checks() -> int:
    sample = build_sample()
    errs = [e.message for e in DOC_V.iter_errors(sample)]
    assert not errs, errs
    assert not semantic_errors(sample), semantic_errors(sample)
    (HERE / "sample_document.json").write_text(json.dumps(sample, indent=2) + "\n", encoding="utf-8")

    # <META - ROLE : Execute mutate | L0-0>
    def mutate(fn):
        d = copy.deepcopy(sample)
        fn(d)
        return d

    cases = {
        "width not multiple of 32": lambda d: d["canvas"].__setitem__("width_px", 100),
        "width above 2048": lambda d: d["canvas"].__setitem__("width_px", 2080),
        "height above 1088": lambda d: d["canvas"].__setitem__("height_px", 1120),
        "tile_px changed": lambda d: d["canvas"].__setitem__("tile_px", 32),
        "unknown blend": lambda d: d["layers"][0].__setitem__("blend", "dissolve"),
        "opacity above 1": lambda d: d["layers"][0].__setitem__("opacity", 1.5),
        "raster layer carrying shapes": lambda d: d["layers"][0].__setitem__("shapes", []),
        "vector layer missing shapes": lambda d: d["layers"][2].pop("shapes"),
        "bezier with 2 points": lambda d: d["layers"][2]["shapes"][1].__setitem__("points", [[0, 0], [1, 1]]),
        "chunk not a PNG": lambda d: d["layers"][0]["raster"]["chunks"][0].__setitem__("png", "AAAA"),
        "cx above 63": lambda d: d["layers"][0]["raster"]["chunks"][0].__setitem__("cx", 64),
        "cy above 33": lambda d: d["layers"][0]["raster"]["chunks"][0].__setitem__("cy", 34),
        "extra property": lambda d: d.__setitem__("order", 1),
        "empty layers": lambda d: d.__setitem__("layers", []),
    }
    for label, fn in cases.items():
        _expect_invalid(label, mutate(fn))

    sem = {
        "duplicate chunk": lambda d: d["layers"][0]["raster"]["chunks"].append(
            dict(d["layers"][0]["raster"]["chunks"][0])),
        "chunk outside canvas": lambda d: d["layers"][0]["raster"]["chunks"].append(
            {"cx": 5, "cy": 0, "png": d["layers"][0]["raster"]["chunks"][0]["png"]}),
        "wrong chunk size": lambda d: d["layers"][0]["raster"]["chunks"][0].__setitem__(
            "png", png_b64(Image.new("RGBA", (16, 16), (1, 2, 3, 255)))),
        "duplicate layer_id": lambda d: d["layers"][1].__setitem__("layer_id", "layer-0001"),
        "dangling active_layer_id": lambda d: d.__setitem__("active_layer_id", "nope"),
    }
    for label, fn in sem.items():
        bad = mutate(fn)
        assert not list(DOC_V.iter_errors(bad)), "schema layer should pass: " + label
        assert semantic_errors(bad), "semantic rule missed: " + label

    flat = flatten(sample)
    assert flat.size == (160, 96)
    assert flat.getpixel((40, 10)) == (76, 194, 255, 255), "base layer parity"
    assert flat.getpixel((140, 70)) == (255, 136, 0, 255), "half-tile chunk at (4,2)"
    px = flat.getpixel((5, 5))
    assert px[3] == 255 and px[:3] != (76, 194, 255), "line art (opacity .8) composited over base"
    assert flat.getpixel((100, 5)) == (0, 0, 0, 0), "absent chunk = transparent"

    layer_file = {"schema_version": "2.0.0", "format": "draw_tool.layer",
                  "source_canvas": sample["canvas"], "layer": sample["layers"][1]}
    assert not list(LAYER_V.iter_errors(layer_file))
    assert json.loads(json.dumps(layer_file, sort_keys=True)) == json.loads(json.dumps(layer_file, sort_keys=True))
    print("schema v2 self-check OK: %d schema-negative, %d semantic-negative, flatten parity, v1 migration"
          % (len(cases), len(sem)))
    return 0

# <META - ROLE : Execute bench | L208-233>
def bench() -> int:
    """Synthetic 1920x1088 numbers. NOT a substitute for real artwork."""
    rng = random.Random(7)
    n = 60 * 34
    scenarios = {}
    noise = bytes(rng.getrandbits(8) for _ in range(CHUNK * CHUNK * 4))
    noise_png = png_b64(Image.frombytes("RGBA", (CHUNK, CHUNK), noise))
    scenarios["worst: every chunk incompressible noise"] = (n, noise_png)
    flat_png = png_b64(_solid((30, 60, 90, 255)))
    scenarios["flat color, every chunk filled"] = (n, flat_png)
    sketch = Image.new("RGBA", (CHUNK, CHUNK), (0, 0, 0, 0))
    for i in range(CHUNK):
        sketch.putpixel((i, (i * 3) % CHUNK), (0, 0, 0, 255))
    scenarios["sketch: 15% of chunks, thin lines"] = (int(n * 0.15), png_b64(sketch))
    for label, (count, b64) in scenarios.items():
        chunks = [{"cx": i % 60, "cy": i // 60, "png": b64} for i in range(count)]
        t0 = time.perf_counter()
        text = json.dumps({"chunks": chunks})
        t1 = time.perf_counter()
        json.loads(text)
        t2 = time.perf_counter()
        print("%-44s chunks=%5d json=%7.1f MB  dump=%.2fs  parse=%.2fs"
              % (label, count, len(text) / 1e6, t1 - t0, t2 - t1))
    print("raw RGBA per full 1920x1088 layer: %.1f MB" % (1920 * 1088 * 4 / 1e6))
    print("raw RGBA per 32x32 chunk: %d bytes" % (CHUNK * CHUNK * 4))
    return 0

if __name__ == "__main__":
    raise SystemExit(bench() if "--bench" in sys.argv else run_checks())
