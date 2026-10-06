#!/usr/bin/env python3
# <META - FILE SUMMARY - Declarative JSON recipe engine: 5-layer dispatch and CLI backend>
"""Layer 5 interpreter: declarative JSON recipe engine and sole CLI backend.

tools/assets/v2/engine.py

Five-layer dispatch (normalize -> primitives/color -> compositor ->
raster_ops -> export/atlas) plus legacy-compat entry points. ``core/``
bridges delegate here through function-local lazy imports, so this
module must never import ``core.canvas`` at top level (templates and
list-ops stay function-local in ``main``). ``fillet_polygon`` stays
canonical in ``core.geometry`` and is reused here, mirroring
``v2.primitives``. Global ``random`` is never used.
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import sys
from pathlib import Path
from typing import Any

from PIL import Image, ImageChops, ImageDraw

from dev.tools.assets.core.drawing import _draw_hud_brackets, _draw_sheen_overlay
from dev.tools.assets.core.geometry import (
    fillet_polygon,
    render_layer_commands,
    render_visor_layer,
    resolve_color,
)
from dev.tools.assets.core.paths import dst, src
from dev.tools.assets.v2 import color as _color
from dev.tools.assets.v2 import compositor as _comp
from dev.tools.assets.v2 import primitives as _prim
from dev.tools.assets.v2 import raster_ops as _filters

_DEFAULT_SPECS: dict[str, dict[str, Any]] = {
    "C-001": {"body_color": "#F4B41A", "visor_color": "#000000", "eye_color": "#FFE84D", "glow_color": "#FFD700", "shadow_rx": 28, "shadow_ry": 6, "shadow_cy": 110},
    "T-002": {"body_color": "#E52B50", "visor_color": "#000000", "eye_color": "#FF3355", "glow_color": "#FF1830", "shadow_rx": 30, "shadow_ry": 5, "shadow_cy": 110},
    "R-003": {"body_color": "#22325E", "visor_color": "#000000", "eye_color": "#59A6FF", "glow_color": "#3388FF", "shadow_rx": 28, "shadow_ry": 6, "shadow_cy": 110},
}

# <META - ROLE : Normalize recipe: add default canvas and layer metadata | L48-79>
def normalize_recipe(recipe: dict) -> dict:
    """Zero-boilerplate normalize: root commands gain a default canvas layer."""
    data: dict = copy.deepcopy(recipe)
    canvas: dict = dict(data.get("canvas", {}))
    canvas.setdefault("width", 128)
    canvas.setdefault("height", 128)
    canvas.setdefault("background", [0, 0, 0, 0])
    data["canvas"] = canvas
    layers: Any = data.get("layers")
    if isinstance(layers, list) and layers:
        normed: list[dict] = []
        for index, entry in enumerate(layers):
            if isinstance(entry, dict) and "commands" in entry:
                layer: dict = dict(entry)
                layer.setdefault("id", f"layer_{index}")
                layer.setdefault("visible", True)
                layer.setdefault("op", "draw")
                layer.setdefault("blend", "normal")
                layer.setdefault("opacity", 1.0)
                normed.append(layer)
            else:
                normed.append(entry)
        data["layers"] = normed
    elif isinstance(data.get("commands"), list):
        data["layers"] = [{
            "id": "implicit", "visible": True, "op": "draw",
            "blend": "normal", "opacity": 1.0,
            "commands": data["commands"],
        }]
    else:
        data.setdefault("layers", [])
    return data

# <META - ROLE : Clone command with all geometric keys reflected across axis | L82-98>
def _mirror_single(cmd: dict, axis: str, pos: float) -> dict:
    """Clone one command with every geometric key reflected across the axis."""
    out: dict = dict(cmd)
    pts_key: str | None = "pts" if isinstance(out.get("pts"), list) else ("points" if isinstance(out.get("points"), list) else None)
    if pts_key is not None:
        out[pts_key] = [[q[0], q[1]] for q in (_prim.reflect_point((float(p[0]), float(p[1])), axis, pos) for p in out[pts_key])]
    seg_key: str | None = "segments" if isinstance(out.get("segments"), list) else ("lines" if isinstance(out.get("lines"), list) else None)
    if seg_key is not None:
        out[seg_key] = [[[r[0], r[1]] for r in (_prim.reflect_point((float(e[0]), float(e[1])), axis, pos) for e in seg)] for seg in out[seg_key]]
    box_key: str | None = "bbox" if isinstance(out.get("bbox"), list) else ("box" if isinstance(out.get("box"), list) else None)
    if box_key is not None:
        out[box_key] = [[[q[0], q[1]] for q in (_prim.reflect_point((float(c[0]), float(c[1])), axis, pos) for c in out[box_key])]]
    for key in ("center", "pos", "start"):
        if isinstance(out.get(key), list) and len(out[key]) >= 2:
            moved: tuple[float, float] = _prim.reflect_point((float(out[key][0]), float(out[key][1])), axis, pos)
            out[key] = [moved[0], moved[1]]
    return out

# <META - ROLE : Expand mirror command into welded polygon or original+reflected pair | L101-118>
def process_mirror_command(cmd: dict, canvas_size: tuple[int, int]) -> list[dict]:
    """Expand mirror into a welded polygon or an original+reflected pair."""
    mirror: Any = cmd.get("mirror")
    if not isinstance(mirror, dict):
        return [cmd]
    axis: str = str(mirror.get("axis", "x"))
    default_pos: float = canvas_size[0] / 2.0 if axis == "x" else canvas_size[1] / 2.0
    pos: float = float(mirror.get("axis_pos", default_pos))
    base: dict = dict(cmd)
    base.pop("mirror", None)
    if str(base.get("type", base.get("cmd", ""))) == "polygon" and bool(mirror.get("weld", True)):
        raw: Any = base.get("pts", base.get("points", []))
        pts: list[tuple[float, float]] = [(float(p[0]), float(p[1])) for p in raw]
        welded: list[tuple[float, float]] = _prim.mirror_points(pts, axis, pos, weld=True)
        key: str = "pts" if "pts" in base else "points"
        base[key] = [[p[0], p[1]] for p in welded]
        return [base]
    return [base, _mirror_single(base, axis, pos)]

# <META - ROLE : Merge SQLite visual-spec rows over defaults keyed by enemy IDs | L121-141>
def _merge_spec_rows(rows: Any, defaults: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    """Merge SQLite visual-spec rows over defaults keyed by known enemy IDs."""
    specs: dict[str, dict[str, Any]] = {}
    for row in rows:
        sid: str = str(row.get("shape_id", "") if isinstance(row, dict) else row["shape_id"])
        vid: str = str(row.get("visual_id", "") if isinstance(row, dict) else row["visual_id"])
        key: str = sid if sid in defaults else (vid if vid in defaults else "")
        if not key:
            continue
        base: dict[str, Any] = defaults[key]
        get: Any = (lambda k, d: row.get(k, d)) if isinstance(row, dict) else (lambda k, d: row[k] if k in row.keys() else d)
        specs[key] = {
            "body_color": str(get("body_color_hex", base["body_color"])),
            "visor_color": str(get("visor_color_hex", base["visor_color"])),
            "eye_color": str(get("eye_color_hex", base["eye_color"])),
            "glow_color": str(get("glow_color_hex", base["glow_color"])),
            "shadow_rx": int(get("shadow_rx", base["shadow_rx"])),
            "shadow_ry": int(get("shadow_ry", base["shadow_ry"])),
            "shadow_cy": int(get("shadow_cy", base["shadow_cy"])),
        }
    return specs

# <META - ROLE : Load visual specs from SQLite with fallback to defaults | L144-156>
def _load_default_specs() -> dict[str, dict[str, Any]]:
    """SQLite merge on success, full DEFAULT_SPECS on empty or failure."""
    defaults: dict[str, dict[str, Any]] = {k: dict(v) for k, v in _DEFAULT_SPECS.items()}
    try:
        from dev.tools.data.config import PROJECT_ROOT
        from dev.tools.data.database import build_database, query_database
        database: Any = build_database(PROJECT_ROOT)
        rows: Any = query_database(database, "SELECT * FROM entity_keybot_visual_specs ORDER BY rowid")
        specs: dict[str, dict[str, Any]] = _merge_spec_rows(rows, defaults)
        return specs if specs else defaults
    except Exception as error:
        print(f"[WARN] SQLite 로드 실패, 기본값 사용: {error}")
        return defaults

# <META - ROLE : Load visual specs from explicit JSON or SQLite source path | L159-179>
def _load_specs_source(source: str) -> dict[str, Any]:
    """Explicit --specs path: JSON dict wins, SQLite DB merged, else empty."""
    path: Path = Path(source)
    if path.exists() and path.suffix.lower() == ".json":
        try:
            data: Any = json.loads(path.read_text(encoding="utf-8"))
            return data if isinstance(data, dict) else {}
        except Exception:
            return {}
    try:
        import sqlite3
        conn: Any = sqlite3.connect(str(path))
        conn.row_factory = sqlite3.Row
        try:
            rows: Any = conn.execute("SELECT * FROM entity_keybot_visual_specs ORDER BY rowid").fetchall()
            defaults: dict[str, dict[str, Any]] = {k: dict(v) for k, v in _DEFAULT_SPECS.items()}
            return _merge_spec_rows(rows, defaults)
        finally:
            conn.close()
    except Exception:
        return {}

# <META - ROLE : Select visual spec source via --specs flag or defaults | L182-189>
def resolve_specs(source: str | None) -> dict:
    """Select the spec DB via --specs; None keeps the DEFAULT_SPECS chain."""
    if source is None:
        return _load_default_specs()
    loaded: dict[str, Any] = _load_specs_source(source)
    if loaded:
        return loaded
    return {k: dict(v) for k, v in _DEFAULT_SPECS.items()}

# <META - ROLE : Resolve export path via dst() SSOT with override support | L192-205>
def resolve_export_path(recipe: dict, out_override: str | None) -> Path | None:
    """Export out via dst() SSOT; override wins verbatim; absent is in-memory."""
    if out_override:
        return Path(out_override)
    export: Any = recipe.get("export")
    if not isinstance(export, dict):
        return None
    out: Any = export.get("out")
    if out is None:
        return None
    candidate: Path = Path(str(out))
    if candidate.is_absolute():
        raise ValueError(f"export.out must be relative (dst SSOT), got absolute: {out!r}")
    return dst(str(out))

# <META - ROLE : Build per-slot palette from specs entry and slot overrides | L208-219>
def _slot_palette(specs: dict, slot: dict) -> dict[str, Any]:
    """Per-slot palette: matching specs entry, else flat specs, slot wins."""
    base: dict[str, Any] = {}
    sid: Any = slot.get("id", slot.get("spec"))
    if isinstance(sid, str) and isinstance(specs.get(sid), dict):
        base.update(specs[sid])
    elif specs and all(not isinstance(v, dict) for v in specs.values()):
        base.update(specs)
    extra: Any = slot.get("palette")
    if isinstance(extra, dict):
        base.update(extra)
    return base

# <META - ROLE : Render one atlas slot from inline layers or recipe ref | L222-245>
def _render_slot(slot: dict, specs: dict, slot_px: int) -> Image.Image:
    """Render one atlas slot from inline layers or a recipe ref file."""
    layers: Any = slot.get("layers")
    if layers is None and isinstance(slot.get("recipe"), str):
        ref_path: Path = Path(str(slot["recipe"]))
        if not ref_path.is_absolute():
            ref_path = src(str(slot["recipe"]))
        ref: dict[str, Any] | None = load_shape_file(ref_path)
        if ref is None:
            raise ValueError(f"atlas slot recipe not found: {slot['recipe']!r}")
        layers = ref.get("layers", ref.get("commands", []))
    if not isinstance(layers, list):
        raise ValueError(f"atlas slot needs inline layers or a recipe ref: {slot!r}")
    sub: dict = {
        "canvas": {"width": slot_px, "height": slot_px, "background": [0, 0, 0, 0]},
        "layers": copy.deepcopy(layers),
        "palette": _slot_palette(specs, slot),
    }
    seed: Any = slot.get("seed")
    if isinstance(seed, int):
        for layer in sub["layers"]:
            if isinstance(layer, dict) and isinstance(layer.get("filter"), dict):
                layer["filter"].setdefault("seed", seed)
    return render_recipe_v2(sub)

# <META - ROLE : Tile explicit atlas slots into columns grid of slot squares | L248-264>
def compose_atlas(recipe: dict, specs: dict) -> Image.Image:
    """Tile explicit export.atlas slots into a columns grid of slot squares."""
    export: Any = recipe.get("export", {})
    atlas: Any = export.get("atlas", {}) if isinstance(export, dict) else {}
    slots: Any = atlas.get("slots", []) if isinstance(atlas, dict) else []
    if not isinstance(slots, list) or not slots:
        raise ValueError("export.atlas.slots must be a non-empty list")
    slot_px: int = int(atlas.get("slot", 128))
    columns: int = max(1, int(atlas.get("columns", 1)))
    rows: int = math.ceil(len(slots) / columns)
    sheet: Image.Image = Image.new("RGBA", (columns * slot_px, rows * slot_px), (0, 0, 0, 0))
    for index, slot in enumerate(slots):
        tile: Image.Image = _render_slot(slot, specs, slot_px)
        if tile.size != (slot_px, slot_px):
            tile = tile.resize((slot_px, slot_px), Image.Resampling.BICUBIC)
        sheet.paste(tile, ((index % columns) * slot_px, (index // columns) * slot_px), tile)
    return sheet

# <META - ROLE : Expand spokes, concentric_shapes, and ring into drawable dicts | L267-293>
def _expand_spokes_rings(cmd: dict) -> list[dict]:
    """Expand spokes, concentric_shapes, and ring into drawable dicts."""
    shape: str = str(cmd.get("type", ""))
    if shape == "spokes":
        center: tuple[float, float] = (float(cmd["center"][0]), float(cmd["center"][1]))
        pairs: list[tuple[tuple[float, float], tuple[float, float]]] = _prim.make_spokes(center, int(cmd.get("count", 8)), float(cmd.get("length", 10.0)), float(cmd.get("inner_radius", 0.0)), float(cmd.get("angle_offset", 0.0)))
        out: list[dict] = []
        for start_pt, end_pt in pairs:
            seg: dict = dict(cmd)
            seg["type"] = "line"
            seg["points"] = [[start_pt[0], start_pt[1]], [end_pt[0], end_pt[1]]]
            out.append(seg)
        return out
    if shape == "concentric_shapes":
        base_cmd: dict = dict(cmd)
        base_cmd["type"] = str(cmd.get("shape", cmd.get("base", "circle")))
        return _prim.make_concentric_shapes(base_cmd, int(cmd.get("count", 2)), float(cmd.get("gap", 4.0)))
    if shape == "ring":
        ring_cmd: dict = dict(cmd)
        outer: float = float(cmd.get("radius", cmd.get("outer", 10.0)))
        thick: float = float(cmd.get("thickness", cmd.get("width", 2.0)))
        ring_cmd["type"] = "circle"
        ring_cmd["radius"] = outer - thick / 2.0
        ring_cmd["fill"] = None
        ring_cmd["width"] = int(round(thick))
        return [ring_cmd]
    return [cmd]

# <META - ROLE : Expand rotated_ellipse and star into polygon samples | L296-331>
def _expand_curved(cmd: dict) -> list[dict]:
    """Expand rotated_ellipse and star into polygon samples (degrees in)."""
    shape: str = str(cmd.get("type", ""))
    if shape == "rotated_ellipse":
        cx0: float = float(cmd["center"][0])
        cy0: float = float(cmd["center"][1])
        rx0: float = float(cmd.get("rx", cmd.get("radius", 10.0)))
        ry0: float = float(cmd.get("ry", rx0))
        rot: float = math.radians(float(cmd.get("rotate", cmd.get("angle", 0.0))))
        corners: list[list[float]] = []
        for i in range(48):
            ang: float = 2.0 * math.pi * float(i) / 48.0
            ex: float = rx0 * math.cos(ang)
            ey: float = ry0 * math.sin(ang)
            corners.append([cx0 + ex * math.cos(rot) - ey * math.sin(rot), cy0 + ex * math.sin(rot) + ey * math.cos(rot)])
        poly: dict = dict(cmd)
        poly["type"] = "polygon"
        poly["points"] = corners
        return [poly]
    if shape == "star":
        sc: tuple[float, float] = (float(cmd["center"][0]), float(cmd["center"][1]))
        raw_radii: Any = cmd.get("radii")
        outer_r: float = float(raw_radii[0]) if isinstance(raw_radii, list) and raw_radii else float(cmd.get("radius", 10.0))
        inner_r: float = float(raw_radii[1]) if isinstance(raw_radii, list) and len(raw_radii) > 1 else outer_r * 0.45
        count: int = int(cmd.get("sides", cmd.get("count", 5)))
        off: float = float(cmd.get("angle_offset", 0.0))
        star_pts: list[list[float]] = []
        for i in range(count * 2):
            rad: float = outer_r if i % 2 == 0 else inner_r
            ang2: float = math.radians(off + 360.0 * float(i) / float(count * 2))
            star_pts.append([sc[0] + rad * math.cos(ang2), sc[1] + rad * math.sin(ang2)])
        star_cmd: dict = dict(cmd)
        star_cmd["type"] = "polygon"
        star_cmd["points"] = star_pts
        return [star_cmd]
    return [cmd]

# <META - ROLE : Route novel shape types to their polygon or line expansions | L334-340>
def _expand_novel(cmd: dict) -> list[dict]:
    """Route novel shape types to their polygon/line expansions."""
    first: list[dict] = _expand_spokes_rings(cmd)
    out: list[dict] = []
    for item in first:
        out.extend(_expand_curved(item))
    return out

# <META - ROLE : Apply affine transform to command points, bbox, and center | L343-362>
def _apply_cmd_transform(cmd: dict) -> dict:
    """Affine point mapping; bbox corners ride along for translate/scale."""
    transform: Any = cmd.get("transform")
    if not isinstance(transform, dict):
        return cmd
    out: dict = dict(cmd)
    for key in ("pts", "points"):
        if isinstance(out.get(key), list):
            raw: Any = out[key]
            out[key] = [[p[0], p[1]] for p in _prim.transform_points([(float(p[0]), float(p[1])) for p in raw], transform)]
    for key in ("bbox", "box"):
        if isinstance(out.get(key), list):
            raw_box: Any = out[key]
            corners: list[tuple[float, float]] = [(float(raw_box[0][0]), float(raw_box[0][1])), (float(raw_box[1][0]), float(raw_box[1][1]))]
            moved_box: list[tuple[float, float]] = _prim.transform_points(corners, transform)
            out[key] = [[moved_box[0][0], moved_box[0][1]], [moved_box[1][0], moved_box[1][1]]]
    if isinstance(out.get("center"), list) and len(out["center"]) >= 2:
        moved_c: list[tuple[float, float]] = _prim.transform_points([(float(out["center"][0]), float(out["center"][1]))], transform)
        out["center"] = [moved_c[0][0], moved_c[0][1]]
    return out

# <META - ROLE : Expand command through mirror, polar, novel, and affine stages | L365-378>
def _expand_command(cmd: dict, canvas_size: tuple[int, int]) -> list[dict]:
    """Mirror, polar-length, novel-shape, and affine expansion for one command."""
    work: dict = dict(cmd)
    if "type" not in work and "cmd" in work:
        work["type"] = work["cmd"]
    out: list[dict] = []
    for item in process_mirror_command(work, canvas_size):
        if isinstance(item.get("start"), list) and "length" in item and "angle" in item and "points" not in item and "pts" not in item:
            end: tuple[float, float] = _prim.polar_to_cartesian((float(item["start"][0]), float(item["start"][1])), float(item["length"]), float(item["angle"]))
            item = dict(item)
            item["points"] = [list(item["start"]), [end[0], end[1]]]
        for novel in _expand_novel(item):
            out.append(_apply_cmd_transform(novel))
    return out

# <META - ROLE : Resolve fill, outline, color, and stroke_fill to RGBA tuples | L381-392>
def _resolve_cmd_colors(cmd: dict, palette: dict[str, Any]) -> dict:
    """Resolve fill/outline/color/stroke_fill literals and $tokens to RGBA."""
    out: dict = dict(cmd)
    for key in ("fill", "outline", "color", "stroke_fill"):
        if key not in out or out[key] is None:
            continue
        val: Any = out[key]
        if isinstance(val, str) and val.strip().startswith("$"):
            out[key] = _color.resolve_color_token(val, palette, {})
        else:
            out[key] = _color.parse_color(val)
    return out

# <META - ROLE : Resolve colors per command and dispatch expanded shapes to primitives | L395-406>
def _render_command(draw: ImageDraw.ImageDraw, cmd: dict, palette: dict[str, Any], canvas_size: tuple[int, int]) -> None:
    """Resolve colors per command and dispatch expanded shapes to primitives."""
    for item in _expand_command(cmd, canvas_size):
        try:
            work: dict = _resolve_cmd_colors(item, palette)
        except Exception as error:
            print(f"  [WARN] 색상 해석 실패 ({item.get('type', '?')}): {error}")
            continue
        try:
            _prim.draw_primitive(draw, work)
        except ValueError as error:
            print(f"  [WARN] 알 수 없는 type: {work.get('type', '')!r} ({error})")

# <META - ROLE : Apply raster affine transform to layer overlay image | L409-431>
def _transform_layer(img: Image.Image, transform: dict) -> Image.Image:
    """Raster affine for a layer overlay (translate exact, rest via AFFINE)."""
    raw_t: Any = transform.get("translate", [0.0, 0.0])
    dx: float = float(raw_t[0]) if isinstance(raw_t, list) and len(raw_t) >= 2 else 0.0
    dy: float = float(raw_t[1]) if isinstance(raw_t, list) and len(raw_t) >= 2 else 0.0
    angle: float = math.radians(float(transform.get("rotate", 0.0)))
    raw_s: Any = transform.get("scale", [1.0, 1.0])
    sx: float = float(raw_s[0]) if isinstance(raw_s, list) and len(raw_s) >= 2 else 1.0
    sy: float = float(raw_s[1]) if isinstance(raw_s, list) and len(raw_s) >= 2 else 1.0
    raw_f: Any = transform.get("flip", [False, False])
    if isinstance(raw_f, list) and len(raw_f) >= 2:
        if bool(raw_f[0]):
            sx = -sx
        if bool(raw_f[1]):
            sy = -sy
    if sx == 0.0:
        sx = 1.0
    if sy == 0.0:
        sy = 1.0
    cos_a: float = math.cos(angle)
    sin_a: float = math.sin(angle)
    coeffs: tuple[float, float, float, float, float, float] = (cos_a / sx, sin_a / sx, -dx, -sin_a / sy, cos_a / sy, -dy)
    return img.transform(img.size, Image.AFFINE, coeffs, resample=Image.Resampling.BICUBIC)

_WHITE: tuple[int, int, int, int] = (255, 255, 255, 255)

# <META - ROLE : Create erase-mask copy with all colors set to white | L436-442>
def _whiten(cmd: dict) -> dict:
    """Erase-mask copy: coverage paints white regardless of op colors."""
    out: dict = dict(cmd)
    out["fill"] = _WHITE
    out["outline"] = _WHITE
    out["color"] = _WHITE
    return out

# <META - ROLE : Render legacy canvas op: draw in place or erase via mask | L445-453>
def _render_legacy_op(base: Image.Image, op: dict, palette: dict[str, Any], canvas_size: tuple[int, int]) -> Image.Image:
    """Legacy canvas op: draw in place, erase via alpha-mask subtraction."""
    if str(op.get("op", "draw")) == "erase":
        mask_img: Image.Image = Image.new("RGBA", base.size, (0, 0, 0, 0))
        _render_command(ImageDraw.Draw(mask_img), _whiten(dict(op, op="draw")), palette, canvas_size)
        _comp.erase_layer_mask(base, mask_img.getchannel("A"))
        return base
    _render_command(ImageDraw.Draw(base), op, palette, canvas_size)
    return base

# <META - ROLE : Composite one normalized layer with commands, filter, and transform | L456-478>
def _render_layer(base: Image.Image, layer: dict, palette: dict[str, Any], canvas_size: tuple[int, int]) -> Image.Image:
    """Composite one normalized layer: command stack or legacy op dict."""
    if not isinstance(layer, dict):
        return base
    if layer.get("visible") is False:
        return base
    if "commands" not in layer:
        return _render_legacy_op(base, layer, palette, canvas_size)
    overlay: Image.Image = Image.new("RGBA", base.size, (0, 0, 0, 0))
    pen: ImageDraw.ImageDraw = ImageDraw.Draw(overlay)
    for cmd in layer.get("commands", []):
        if isinstance(cmd, dict):
            _render_command(pen, cmd if str(layer.get("op", "draw")) != "erase" else _whiten(cmd), palette, canvas_size)
    filt: Any = layer.get("filter")
    if isinstance(filt, dict) and filt:
        overlay = _filters.apply_filters(overlay, filt)
    transform: Any = layer.get("transform")
    if isinstance(transform, dict) and transform:
        overlay = _transform_layer(overlay, transform)
    if str(layer.get("op", "draw")) == "erase":
        _comp.erase_layer_mask(base, overlay.getchannel("A"))
        return base
    return _comp.blend_layers(base, overlay, str(layer.get("blend", "normal")), float(layer.get("opacity", 1.0)))

# <META - ROLE : Render full 5-layer recipe, optionally writing to disk | L481-496>
def render_recipe_v2(recipe: dict, output_path: Path | None = None) -> Image.Image:
    """Full 5-layer render; writes to disk only when output_path is given."""
    norm: dict = normalize_recipe(recipe)
    canvas: dict = norm.get("canvas", {})
    size: tuple[int, int] = (int(canvas.get("width", 128)), int(canvas.get("height", 128)))
    bg: tuple[int, int, int, int] | None = _color.parse_color(canvas.get("background", [0, 0, 0, 0]))
    img: Image.Image = Image.new("RGBA", size, bg if bg is not None else (0, 0, 0, 0))
    palette: dict[str, Any] = dict(norm.get("palette", {}) or {})
    for layer in norm.get("layers", []):
        if isinstance(layer, dict):
            img = _render_layer(img, layer, palette, size)
    if output_path is not None:
        dest: Path = Path(output_path)
        dest.parent.mkdir(parents=True, exist_ok=True)
        img.save(dest, "PNG")
    return img

# <META - ROLE : Legacy-compatible recipe render: write PNG and return path | L499-521>
def run_recipe_compat(recipe: dict, output_override: str | None = None, size_override: str | None = None) -> Path:
    """Legacy run_recipe observable behavior: PNG written, Path returned."""
    work: dict = copy.deepcopy(recipe)
    if size_override:
        parts: list[str] = size_override.lower().split("x")
        work["canvas"] = dict(work.get("canvas", {}))
        work["canvas"]["width"] = int(parts[0])
        work["canvas"]["height"] = int(parts[1])
    if output_override:
        dest: Path = Path(output_override)
    else:
        export: Any = work.get("export")
        if isinstance(export, dict) and export.get("out"):
            resolved: Path | None = resolve_export_path(work, None)
            dest = resolved if resolved is not None else Path("output.png")
        elif isinstance(work.get("output"), str):
            dest = Path(str(work["output"]))
        else:
            dest = Path("output.png")
    img: Image.Image = render_recipe_v2(work)
    dest.parent.mkdir(parents=True, exist_ok=True)
    img.save(dest, "PNG")
    return dest

# <META - ROLE : Compose export.atlas grid and write unless export is silent | L524-533>
def _main_atlas(norm: dict, specs: dict, out_override: str | None) -> None:
    """Compose the export.atlas grid and write it unless export is silent."""
    sheet: Image.Image = compose_atlas(norm, specs)
    dest: Path | None = resolve_export_path(norm, out_override)
    if dest is None:
        print(f"[OK] 인메모리 아틀라스 {sheet.size} (export.out 없음 — 기록 없음)")
        return
    dest.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(dest, "PNG")
    print(f"[OK] → {dest}")

# <META - ROLE : CLI backend: parse argv, load recipe, render, and export | L536-579>
def main() -> None:
    """Sole CLI backend: legacy argv plus --specs/--out; silent without export."""
    parser: argparse.ArgumentParser = argparse.ArgumentParser(description="선언형 캔버스 렌더러 — JSON 레시피로 PNG 출력 (v2 엔진)")
    parser.add_argument("recipe", nargs="?", help="JSON 레시피 파일 경로")
    parser.add_argument("-o", "--output", "--out", dest="out", help="출력 PNG 경로 (레시피 설정 덮어쓰기)")
    parser.add_argument("--canvas", metavar="WxH", help="캔버스 크기 덮어쓰기 (예: 256x256)")
    parser.add_argument("--specs", default=None, help="시각 스펙 DB/JSON 경로 (미지정 시 DEFAULT_SPECS 폴백)")
    parser.add_argument("--template", nargs="?", const="all", metavar="NAME", help="예제 JSON 출력")
    parser.add_argument("--list-ops", action="store_true", help="지원 연산 목록 출력")
    args: argparse.Namespace = parser.parse_args()
    if args.list_ops:
        from dev.tools.assets.core.canvas import LIST_OPS_TEXT
        print(LIST_OPS_TEXT)
        return
    if args.template is not None:
        from dev.tools.assets.core.canvas import TEMPLATES
        name: str = args.template if args.template in TEMPLATES else "all"
        if args.template not in TEMPLATES:
            print(f"[WARN] '{args.template}' 없음. 사용 가능: {', '.join(TEMPLATES)}\n")
        print(json.dumps(TEMPLATES[name], indent=2, ensure_ascii=False))
        return
    if not args.recipe:
        parser.print_help()
        return
    path: Path = Path(args.recipe)
    if not path.exists():
        print(f"[ERROR] 파일 없음: {path}")
        sys.exit(1)
    data: dict[str, Any] | None = load_shape_file(path)
    if data is None:
        print(f"[ERROR] 레시피 로드 실패: {path}")
        sys.exit(1)
    specs: dict = resolve_specs(args.specs)
    norm: dict = normalize_recipe(data)
    export: Any = norm.get("export")
    atlas: Any = export.get("atlas") if isinstance(export, dict) else None
    if isinstance(atlas, dict) and isinstance(atlas.get("slots"), list):
        _main_atlas(norm, specs, args.out)
        return
    merged: dict = copy.deepcopy(data)
    if specs and "palette" not in merged and all(not isinstance(v, dict) for v in specs.values()):
        merged["palette"] = dict(specs)
    out: Path = run_recipe_compat(merged, output_override=args.out, size_override=args.canvas)
    print(f"[OK] → {out}")

# <META - ROLE : Load JSON geometry spec with base-template inheritance | L582-625>
def load_shape_file(file_path: str | Path) -> dict[str, Any] | None:
    """JSON geometry spec loader with base-template inheritance (canonical port)."""
    p: Path = Path(file_path)
    if not p.exists():
        return None
    try:
        with open(p, "r", encoding="utf-8") as f:
            data: Any = json.load(f)
    except json.JSONDecodeError:
        try:
            import re
            text: str = p.read_text(encoding="utf-8")
            clean_text: str = re.sub(r"/\*.*?\*/", "", re.sub(r"//.*", "", text), flags=re.DOTALL)
            data = json.loads(clean_text)
        except Exception as e:
            print(f"[WARN] 지오메트리 JSON 로드 실패 ({p}): {e}")
            return None
    except Exception as e:
        print(f"[WARN] 파일 읽기 실패 ({p}): {e}")
        return None
    base_names: list[str] = []
    if isinstance(data, dict):
        if "bases" in data and isinstance(data["bases"], list):
            base_names.extend([str(b) for b in data["bases"]])
        if "keycap_base" in data:
            base_names.append(str(data["keycap_base"]))
        if "visor_base" in data:
            base_names.append(str(data["visor_base"]))
        if "base" in data:
            base_names.append(str(data["base"]))
    if base_names:
        merged: dict[str, Any] = {}
        for b_name in base_names:
            b_file: Path = p.parent / b_name if b_name.endswith(".json") else p.parent / f"{b_name}.json"
            b_spec: dict[str, Any] | None = load_shape_file(b_file)
            if b_spec and isinstance(b_spec, dict):
                for k, v in b_spec.items():
                    if k not in ("base", "bases", "keycap_base", "visor_base"):
                        merged[k] = copy.deepcopy(v)
        for k, v in data.items():
            if k not in ("base", "bases", "keycap_base", "visor_base"):
                merged[k] = copy.deepcopy(v)
        return merged
    return data

# <META - ROLE : Render legacy closed shapes: polygon, rounded_rect, circle, chord | L628-652>
def _compat_closed(d: ImageDraw.ImageDraw, cmd: dict[str, Any], spec: dict[str, Any], c_type: str, is_gray: bool) -> None:
    """Legacy closed-shape branches: polygon, rounded_rect, circle, chord."""
    if c_type == "polygon":
        raw_pts: list[Any] = [tuple(p) for p in cmd.get("pts", cmd.get("points", []))]
        fillet: float = float(cmd.get("fillet", 0.0))
        pts: list[Any] = fillet_polygon(raw_pts, fillet) if fillet > 0.0 else raw_pts
        fill: Any = (255 if is_gray else resolve_color(cmd.get("fill"), spec)) if "fill" in cmd and cmd["fill"] is not None else None
        outline: Any = (255 if is_gray else resolve_color(cmd.get("outline"), spec)) if "outline" in cmd and cmd["outline"] is not None else None
        d.polygon(pts, fill=fill, outline=outline, width=int(cmd.get("width", 1)))
    elif c_type == "rounded_rect":
        box: list[Any] = [tuple(p) for p in cmd["box"]]
        r: int = int(cmd.get("radius", 8))
        fill = (255 if is_gray else resolve_color(cmd.get("fill"), spec)) if "fill" in cmd and cmd["fill"] is not None else None
        outline = (255 if is_gray else resolve_color(cmd.get("outline"), spec)) if "outline" in cmd and cmd["outline"] is not None else None
        d.rounded_rectangle(box, radius=r, fill=fill, outline=outline, width=int(cmd.get("width", 1)))
    elif c_type in ("circle", "ellipse"):
        box = [tuple(p) for p in cmd["box"]]
        fill = (255 if is_gray else resolve_color(cmd.get("fill"), spec)) if "fill" in cmd and cmd["fill"] is not None else None
        outline = (255 if is_gray else resolve_color(cmd.get("outline"), spec)) if "outline" in cmd and cmd["outline"] is not None else None
        d.ellipse(box, fill=fill, outline=outline, width=int(cmd.get("width", 1)))
    elif c_type == "chord":
        box = [tuple(p) for p in cmd["box"]]
        fill = (255 if is_gray else resolve_color(cmd.get("fill"), spec)) if "fill" in cmd and cmd["fill"] is not None else None
        outline = (255 if is_gray else resolve_color(cmd.get("outline"), spec)) if "outline" in cmd and cmd["outline"] is not None else None
        d.chord(box, start=float(cmd.get("start", 0)), end=float(cmd.get("end", 180)), fill=fill, outline=outline, width=int(cmd.get("width", 1)))

# <META - ROLE : Render legacy open shapes: arc, line, lines, rect, text | L655-699>
def _compat_open(d: ImageDraw.ImageDraw, cmd: dict[str, Any], spec: dict[str, Any], c_type: str, is_gray: bool) -> None:
    """Legacy open-shape branches: arc, line, lines, rect, text."""
    if c_type == "arc":
        box: list[Any] = [tuple(p) for p in cmd["box"]]
        col: Any = 255 if is_gray else resolve_color(cmd.get("color", cmd.get("outline", cmd.get("fill"))), spec)
        d.arc(box, start=float(cmd.get("start", 0)), end=float(cmd.get("end", 180)), fill=col, width=int(cmd.get("width", 2)))
    elif c_type == "line":
        pts: list[Any] = [tuple(p) for p in cmd.get("pts", cmd.get("points", []))]
        col = 255 if is_gray else resolve_color(cmd.get("color", cmd.get("fill")), spec)
        d.line(pts, fill=col, width=int(cmd.get("width", 2)))
    elif c_type == "lines":
        segs: Any = cmd.get("segments", cmd.get("lines", []))
        for seg in segs:
            if isinstance(seg, dict):
                seg_pts: list[Any] = [tuple(p) for p in seg.get("points", seg.get("pts", []))]
                seg_col: Any = 255 if is_gray else resolve_color(
                    seg.get("color", seg.get("fill", cmd.get("color", cmd.get("fill")))), spec)
                seg_width: int = int(seg.get("width", cmd.get("width", 2)))
            else:
                seg_pts = [tuple(p) for p in seg]
                seg_col = 255 if is_gray else resolve_color(cmd.get("color", cmd.get("fill")), spec)
                seg_width = int(cmd.get("width", 2))
            if len(seg_pts) >= 2:
                d.line(seg_pts, fill=seg_col, width=seg_width)
    elif c_type in ("rect", "rectangle"):
        box = [tuple(p) for p in cmd.get("box", cmd.get("bbox", []))]
        fill: Any = (255 if is_gray else resolve_color(cmd.get("fill"), spec)) if "fill" in cmd and cmd["fill"] is not None else None
        outline: Any = (255 if is_gray else resolve_color(cmd.get("outline"), spec)) if "outline" in cmd and cmd["outline"] is not None else None
        d.rectangle(box, fill=fill, outline=outline, width=int(cmd.get("width", 1)))
    elif c_type == "text":
        from PIL import ImageFont
        pos: Any = tuple(cmd.get("pos", [64, 64]))
        txt: str = str(cmd.get("text", ""))
        fill = 255 if is_gray else resolve_color(cmd.get("color", cmd.get("fill", "#FFFFFF")), spec)
        font_size: int = int(cmd.get("size", cmd.get("font_size", 44)))
        try:
            font: Any = ImageFont.load_default(size=font_size)
        except Exception:
            font = ImageFont.load_default()
        bbox: Any = d.textbbox((0, 0), txt, font=font)
        tw: int = bbox[2] - bbox[0]
        th: int = bbox[3] - bbox[1]
        stroke_width: int = int(cmd.get("stroke_width", 0))
        stroke_fill: Any = (255 if is_gray else resolve_color(cmd.get("stroke_fill", fill), spec)) if stroke_width > 0 else None
        d.text((pos[0] - tw // 2, pos[1] - th // 2 - 2), txt, fill=fill, font=font, stroke_width=stroke_width, stroke_fill=stroke_fill)

# <META - ROLE : Render single declarative vector command via compat dispatch | L702-713>
def render_command(d: ImageDraw.ImageDraw, cmd: dict[str, Any], spec: dict[str, Any], base_img: Image.Image | None = None) -> None:
    """Single declarative vector command (core bridge canonical port)."""
    if not isinstance(cmd, dict):
        return
    c_type: str = str(cmd.get("cmd", ""))
    if not c_type:
        return
    is_gray: bool = getattr(d, "mode", "RGBA") == "L"
    if c_type in ("polygon", "rounded_rect", "circle", "ellipse", "chord"):
        _compat_closed(d, cmd, spec, c_type, is_gray)
    elif c_type in ("arc", "line", "lines", "rect", "rectangle", "text"):
        _compat_open(d, cmd, spec, c_type, is_gray)

# <META - ROLE : Render inherited layer: base image plus cutout masking and overlays | L716-739>
def _compat_inherited(shape_spec: dict[str, Any], layer_def: dict[str, Any], spec: dict[str, Any], size: tuple[int, int]) -> Image.Image:
    """Inherit-branch: base image plus cutout masking and overlay commands."""
    merged_spec: dict[str, Any] = dict(shape_spec.get("dynamic_colors", {}))
    merged_spec.update(spec)
    base_img: Image.Image = render_shape_layer(shape_spec, layer_def["inherit"], spec, size)
    cutouts: Any = layer_def.get("cutouts", [])
    if cutouts:
        cutout_mask: Image.Image = Image.new("L", size, 255)
        md: ImageDraw.ImageDraw = ImageDraw.Draw(cutout_mask)
        for poly in cutouts:
            pts: list[Any] = [tuple(p) for p in poly]
            if len(pts) >= 3:
                md.polygon(pts, fill=0)
        r: Image.Image
        g: Image.Image
        b: Image.Image
        a: Image.Image
        r, g, b, a = base_img.split()
        a = Image.composite(Image.new("L", size, 0), a, Image.eval(cutout_mask, lambda x: 255 - x))
        base_img.putalpha(a)
    cmds: Any = layer_def.get("commands", [])
    if cmds:
        render_layer_commands(base_img, cmds, merged_spec)
    return base_img

# <META - ROLE : Render plain layer with clip_mask and face overlay post-processing | L742-786>
def _compat_plain(shape_spec: dict[str, Any], layers: Any, lookup_name: str, layer_def: Any, merged_spec: dict[str, Any], size: tuple[int, int]) -> Image.Image:
    """Plain list/dict layer plus clip_mask and face overlay post-processing."""
    img: Image.Image = Image.new("RGBA", size, (0, 0, 0, 0))
    if isinstance(layer_def, list):
        render_layer_commands(img, layer_def, merged_spec)
    elif isinstance(layer_def, dict):
        cmds: Any = layer_def.get("commands", [])
        if cmds:
            render_layer_commands(img, cmds, merged_spec)
    clip_mask_def: Any = shape_spec.get("clip_mask") or (layers.get("clip_mask") if isinstance(layers, dict) else None)
    if clip_mask_def and lookup_name in ("body", "body_100", "body_50", "body_25"):
        mask_img: Image.Image = Image.new("L", size, 0)
        render_command(ImageDraw.Draw(mask_img), clip_mask_def, merged_spec)
        masked_a: Image.Image = ImageChops.multiply(img.getchannel("A"), mask_img)
        img.putalpha(masked_a)
        outline_cmd: dict[str, Any] = dict(clip_mask_def)
        outline_cmd["fill"] = None
        outline_cmd["outline"] = "#000000"
        outline_cmd["width"] = 3
        render_command(ImageDraw.Draw(img), outline_cmd, merged_spec)
    if lookup_name.startswith("face_"):
        v_def: Any = layers.get("visor", {}) if isinstance(layers, dict) else {}
        sheen: Any = v_def.get("sheen") if isinstance(v_def, dict) else None
        v_shape: Any = v_def.get("shape") if isinstance(v_def, dict) else None
        if sheen and v_shape:
            p1: Any = tuple(sheen.get("p1", [42, 44]))
            p2: Any = tuple(sheen.get("p2", [30, 76]))
            dx: int = int(sheen.get("dx", 5))
            # <META - ROLE : Render visor shape into mask draw context for sheen overlay | L0-0>
            def _mask_fn(md: ImageDraw.ImageDraw) -> None:
                render_command(md, v_shape, merged_spec)
            _draw_sheen_overlay(ImageDraw.Draw(img), _mask_fn, p1=p1, p2=p2, dx=dx)
        hud: Any = v_def.get("hud_brackets") if isinstance(v_def, dict) else None
        if hud:
            bx0: int
            by0: int
            bx0, by0 = hud.get("box", [[36, 60], [92, 82]])[0]
            bx1: int
            by1: int
            bx1, by1 = hud.get("box", [[36, 60], [92, 82]])[1]
            arm: int = int(hud.get("arm", 5))
            h_col: Any = tuple(hud.get("color", [255, 255, 255, 235]))
            h_width: int = int(hud.get("width", 1))
            _draw_hud_brackets(ImageDraw.Draw(img), bx0, by0, bx1, by1, arm=arm, color=h_col, width=h_width)
    return img

# <META - ROLE : Render named shape-spec layer via inherit, visor, or plain dispatch | L789-807>
def render_shape_layer(shape_spec: dict[str, Any], layer_name: str, spec: dict[str, Any], size: tuple[int, int] = (128, 128)) -> Image.Image:
    """Named shape-spec layer renderer (core bridge canonical port)."""
    merged_spec: dict[str, Any] = dict(shape_spec.get("dynamic_colors", {}))
    merged_spec.update(spec)
    layers: Any = shape_spec.get("layers", shape_spec)
    lookup_name: str = layer_name
    if isinstance(layers, dict) and lookup_name not in layers:
        if lookup_name == "body_100" and "body" in layers:
            lookup_name = "body"
        elif lookup_name == "visor_100" and "visor" in layers:
            lookup_name = "visor"
        elif lookup_name == "face_idle_100" and "face_neutral" in layers:
            lookup_name = "face_neutral"
    layer_def: Any = layers.get(lookup_name) if isinstance(layers, dict) else None
    if isinstance(layer_def, dict) and "inherit" in layer_def:
        return _compat_inherited(shape_spec, layer_def, spec, size)
    if lookup_name in ("visor", "visor_100") and isinstance(layer_def, dict) and "shape" in layer_def:
        return render_visor_layer(layer_def, merged_spec, size)
    return _compat_plain(shape_spec, layers, lookup_name, layer_def, merged_spec, size)

if __name__ == "__main__":
    main()
