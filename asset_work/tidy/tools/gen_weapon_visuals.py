#!/usr/bin/env python3
# <META - FILE SUMMARY - Generate title weapon silhouettes as a fixed-cell atlas>
"""Generate the title weapon silhouettes as one fixed-cell atlas."""
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

from dev.tools.assets.core import canvas
from dev.tools.assets.core.atlas import AtlasBuilder
from dev.tools.assets.core.geometry import fillet_polygon

Color = tuple[int, int, int, int]
FILLET_RADIUS = 2.5
ATLAS_NAME = "title_weapons_atlas.png"

# <META - ROLE : Parse hex color string to RGBA tuple | L30-32>
def _rgba(value: str) -> Color:
    parsed = canvas._parse_color(value)
    return parsed if parsed is not None else (0, 0, 0, 0)

# <META - ROLE : Render weapon silhouette with filleted polygons into atlas cell | L35-58>
def _render_weapon(
    weapon: dict[str, Any], palette: dict[str, str], size: tuple[int, int]
) -> Image.Image:
    """Render one mechanical weapon silhouette into a fixed atlas cell."""
    image = Image.new("RGBA", size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    outline = _rgba(palette.get("outline", "#0F172A"))
    accent = _rgba(palette.get(weapon.get("accent", "neon_cyan"), "#22D3EE"))
    polygons = weapon.get("polys", [])
    for polygon in polygons:
        raw_points = [tuple(point) for point in polygon.get("pts", [])]
        if len(raw_points) < 3:
            continue
        points = fillet_polygon(raw_points, FILLET_RADIUS)
        tone = _rgba(palette.get(polygon.get("tone", "metal_mid"), "#94A3B8"))
        draw.polygon(points, fill=tone, outline=outline)
    if polygons:
        top_points = fillet_polygon(
            [tuple(point) for point in polygons[0].get("pts", [])], FILLET_RADIUS
        )
        if len(top_points) >= 2:
            edge = (accent[0], accent[1], accent[2], 140)
            draw.line(top_points + [top_points[0]], fill=edge, width=1)
    return image

# <META - ROLE : Render manifest-ordered weapons into a single atlas | L61-72>
def build(profiles_path: Path, output_dir: Path) -> list[Path]:
    """Render manifest-ordered weapons into one three-cell atlas."""
    data = json.loads(profiles_path.read_text(encoding="utf-8"))
    palette: dict[str, str] = data.get("palette", {})
    size_raw = data.get("canvas", [96, 96])
    size = (int(size_raw[0]), int(size_raw[1]))
    images = [
        _render_weapon(weapon, palette, size) for weapon in data.get("weapons", [])
    ]
    output_path = output_dir / ATLAS_NAME
    AtlasBuilder(slot_size=size, columns=len(images)).build(images, output_path)
    return [output_path]

# <META - ROLE : CLI entrypoint for title weapon atlas generation | L75-84>
def main() -> None:
    parser = argparse.ArgumentParser(description="Generate the title weapon atlas")
    parser.add_argument("--profiles", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.profiles.exists():
        print(f"[ERROR] profiles 없음: {args.profiles}")
        sys.exit(1)
    written = build(args.profiles, args.output)
    print(f"[GEN] {len(written)}개 무기 atlas 생성 → {args.output}")

if __name__ == "__main__":
    main()
