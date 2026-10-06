#!/usr/bin/env python3
# <META - FILE SUMMARY - Generate UI surface PNGs and mask sets from a manifest>
"""
tools/assets/generators/gen_ui_surfaces.py

manifest(JSON) → surface PNG + tint/specular/emissive mask 세트를 생성한다.
각 surface의 "kind"에 따라 dispatch하며, 산출물은 --output 디렉토리 하위의
surfaces/ 와 masks/ 로 분리 저장된다.

Usage:
  python -m dev.tools.assets.generators.gen_ui_surfaces \
      --manifest features/ui/stage/data/manifests/hud_surfaces.json \
      --output features/ui/stage/assets/generated/
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

try:
    from PIL import Image, ImageDraw
except ImportError:
    print("[ERROR] Pillow 미설치: pip install Pillow")
    sys.exit(1)

from dev.tools.assets.core import canvas, masks
from dev.tools.assets.core.geometry import (
    get_asymmetric_chamfer_polygon,
    get_bracket_segments,
)
from dev.tools.assets.core.nine_slice import NineSliceBuilder

Color = tuple[int, int, int, int]

# <META - ROLE : Parse hex color string to RGBA tuple | L42-45>
def _rgba(value: str) -> Color:
    """#RRGGBB / #RRGGBBAA → RGBA tuple."""
    parsed = canvas._parse_color(value)
    return parsed if parsed is not None else (0, 0, 0, 0)

# <META - ROLE : Create a new RGBA image and draw context | L48-50>
def _new(size: list[int]) -> tuple[Image.Image, "ImageDraw.ImageDraw"]:
    img = Image.new("RGBA", (int(size[0]), int(size[1])), (0, 0, 0, 0))
    return img, ImageDraw.Draw(img)

# <META - ROLE : Render nine-slice surface with border and notch | L53-60>
def _render_nine_slice(spec: dict[str, Any]) -> Image.Image:
    builder = NineSliceBuilder(
        border=int(spec.get("border", 2)),
        notch=int(spec.get("notch", 4)),
        background=_rgba(spec.get("bg", "#00000000")),
        foreground=_rgba(spec.get("fg", "#FFFFFFFF")),
    )
    return builder.render((int(spec["size"][0]), int(spec["size"][1])))

# <META - ROLE : Render chamfered plate with bevel frame | L63-72>
def _render_chamfer_plate(spec: dict[str, Any]) -> Image.Image:
    img, draw = _new(spec["size"])
    w, h = int(spec["size"][0]), int(spec["size"][1])
    pts = get_asymmetric_chamfer_polygon(
        0, 0, w, h, int(spec.get("chamfer", 12)), spec.get("corners", "TL_BR"),
    )
    draw.polygon(pts, fill=_rgba(spec.get("bg", "#0D0C3AFF")))
    canvas.draw_bevel_frame(draw, pts, _rgba(spec.get("fg", "#22D3EEFF")),
                            _rgba("#00000080"), width=int(spec.get("border", 2)))
    return img

# <META - ROLE : Render visor glass with scanlines and rim | L75-85>
def _render_visor(spec: dict[str, Any]) -> Image.Image:
    img, draw = _new(spec["size"])
    w, h = int(spec["size"][0]), int(spec["size"][1])
    bbox = [(3, 3), (w - 4, h - 4)]
    canvas.fill_visor_glass(draw, bbox, int(spec.get("radius", 10)),
                            _rgba(spec.get("bg", "#071526FF")),
                            _rgba(spec.get("rim", "#0284C7FF")))
    scan = _rgba(spec.get("rim", "#0284C7FF"))
    scan = (scan[0], scan[1], scan[2], int(spec.get("scanline_alpha", 45)))
    canvas.composite_scanlines(draw, bbox, 3, scan)
    return img

# <META - ROLE : Render keycap well with rounded rectangles | L88-97>
def _render_keycap_well(spec: dict[str, Any]) -> Image.Image:
    img, draw = _new(spec["size"])
    w, h = int(spec["size"][0]), int(spec["size"][1])
    inset = int(spec.get("well_inset", 10))
    draw.rounded_rectangle([(2, 2), (w - 3, h - 3)], radius=10,
                           fill=_rgba(spec.get("bg", "#111827FF")),
                           outline=_rgba(spec.get("fg", "#334155FF")), width=2)
    draw.rounded_rectangle([(inset, inset), (w - inset - 1, h - inset - 1)], radius=8,
                           outline=_rgba(spec.get("fg", "#334155FF")), width=1)
    return img

# <META - ROLE : Render keycap with well inset and border | L100-109>
def _render_keycap(spec: dict[str, Any]) -> Image.Image:
    img, draw = _new(spec["size"])
    w, h = int(spec["size"][0]), int(spec["size"][1])
    inset = int(spec.get("well_inset", 12))
    draw.rounded_rectangle([(2, 2), (w - 3, h - 3)], radius=12,
                           fill=_rgba(spec.get("bg", "#F8FAFCFF")),
                           outline=_rgba(spec.get("fg", "#0F172AFF")), width=3)
    draw.rounded_rectangle([(inset, inset), (w - inset - 1, h - inset - 1)], radius=8,
                           fill=_rgba("#F1F5F9FF"), outline=_rgba("#E2E8F0FF"), width=2)
    return img

# <META - ROLE : Render scanline tile pattern with configurable alpha | L112-118>
def _render_scanline_tile(spec: dict[str, Any]) -> Image.Image:
    img, draw = _new(spec["size"])
    w, h = int(spec["size"][0]), int(spec["size"][1])
    col = _rgba(spec.get("fg", "#22D3EEFF"))
    col = (col[0], col[1], col[2], int(spec.get("alpha", 40)))
    canvas.composite_scanlines(draw, [(0, 0), (w - 1, h - 1)], int(spec.get("step", 3)), col)
    return img

# <META - ROLE : Render grid tile pattern with configurable spacing | L121-131>
def _render_grid_tile(spec: dict[str, Any]) -> Image.Image:
    img, draw = _new(spec["size"])
    w, h = int(spec["size"][0]), int(spec["size"][1])
    step = int(spec.get("step", 12))
    col = _rgba(spec.get("fg", "#22D3EEFF"))
    col = (col[0], col[1], col[2], int(spec.get("alpha", 28)))
    for x in range(0, w, step):
        draw.line([(x, 0), (x, h - 1)], fill=col, width=1)
    for y in range(0, h, step):
        draw.line([(0, y), (w - 1, y)], fill=col, width=1)
    return img

# <META - ROLE : Render icon atlas strip with cell outlines | L134-143>
def _render_icon_atlas(spec: dict[str, Any]) -> Image.Image:
    cell = int(spec.get("cell", 64))
    count = int(spec.get("count", 9))
    img = Image.new("RGBA", (cell * count, cell), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    col = _rgba(spec.get("fg", "#F5F1E6FF"))
    for i in range(count):
        ox = i * cell
        draw.rectangle([(ox + 12, 12), (ox + cell - 13, cell - 13)], outline=col, width=2)
    return img

# <META - ROLE : Render bracket frame with arm segments | L146-152>
def _render_bracket(spec: dict[str, Any]) -> Image.Image:
    img, draw = _new(spec["size"])
    w, h = int(spec["size"][0]), int(spec["size"][1])
    col = _rgba(spec.get("fg", "#F5B301FF"))
    for seg in get_bracket_segments(2, 2, w - 3, h - 3, int(spec.get("arm", 10))):
        draw.line(seg, fill=col, width=int(spec.get("width", 2)))
    return img

_DISPATCH = {
    "nine_slice": _render_nine_slice,
    "chamfer_plate": _render_chamfer_plate,
    "visor": _render_visor,
    "keycap_well": _render_keycap_well,
    "keycap": _render_keycap,
    "scanline_tile": _render_scanline_tile,
    "grid_tile": _render_grid_tile,
    "icon_atlas": _render_icon_atlas,
    "bracket": _render_bracket,
}

# <META - ROLE : Dispatch surface spec to appropriate renderer by kind | L167-173>
def render_surface(spec: dict[str, Any]) -> Image.Image:
    """Dispatch a single surface spec to its renderer."""
    kind = spec.get("kind", "")
    fn = _DISPATCH.get(kind)
    if fn is None:
        raise ValueError(f"알 수 없는 surface kind: {kind!r} (지원: {sorted(_DISPATCH)})")
    return fn(spec)

# <META - ROLE : Render all surfaces and masks declared in manifest | L176-195>
def build(manifest_path: Path, output_dir: Path) -> list[Path]:
    """Render all surfaces + masks declared in the manifest. Returns written paths."""
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    surf_dir = output_dir / manifest.get("output_subdir", "surfaces")
    mask_dir = output_dir / manifest.get("mask_subdir", "masks")
    surf_dir.mkdir(parents=True, exist_ok=True)
    mask_dir.mkdir(parents=True, exist_ok=True)

    written: list[Path] = []
    for spec in manifest.get("surfaces", []):
        surface = render_surface(spec)
        surf_path = surf_dir / f"{spec['id']}.png"
        surface.save(surf_path, "PNG")
        written.append(surf_path)
        for mask_kind in spec.get("emit", []):
            mask_img = masks.build_mask(surface, mask_kind)
            mask_path = mask_dir / f"{spec['id']}_{mask_kind}.png"
            mask_img.save(mask_path, "PNG")
            written.append(mask_path)
    return written

# <META - ROLE : CLI entrypoint for UI surface and mask generation | L198-207>
def main() -> None:
    parser = argparse.ArgumentParser(description="Generate UI surface + mask assets from a manifest")
    parser.add_argument("--manifest", type=Path, required=True, help="manifest JSON path")
    parser.add_argument("--output", type=Path, required=True, help="output directory (surfaces/ + masks/)")
    args = parser.parse_args()
    if not args.manifest.exists():
        print(f"[ERROR] manifest 없음: {args.manifest}")
        sys.exit(1)
    written = build(args.manifest, args.output)
    print(f"[GEN] {len(written)}개 파일 생성 → {args.output}")

if __name__ == "__main__":
    main()
