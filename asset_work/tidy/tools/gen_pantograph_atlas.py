#!/usr/bin/env python3
# <META - FILE SUMMARY - Bake pantograph keycap atlas and synchronized core 9-slice assets>
"""Bake clean pantograph keycap atlas and synchronized core 9-slice assets."""

import argparse
import json
import shutil
import sys
from pathlib import Path
from typing import Any

try:
    from PIL import Image, ImageDraw
except ImportError:
    print("[ERROR] Pillow 필요: pip install Pillow")
    sys.exit(1)

from dev.tools.assets.core.paths import CORE_KEYCAP, IMAGES_PANTOGRAPH, PROFILE_PANTOGRAPH, dst, src

JSON_PATH = src(PROFILE_PANTOGRAPH)
OUTPUT_DIR = dst(IMAGES_PANTOGRAPH)
OUTPUT_FILE = OUTPUT_DIR / "sheet.png"
CORE_DIR = dst(CORE_KEYCAP)

# <META - ROLE : Convert hex color code to RGBA tuple | L26-32>
def _rgba(hex_code: str, alpha: int = 255) -> tuple[int, int, int, int]:
    value = hex_code.lstrip("#")
    if len(value) == 6:
        return (int(value[0:2], 16), int(value[2:4], 16), int(value[4:6], 16), alpha)
    if len(value) == 8:
        return (int(value[0:2], 16), int(value[2:4], 16), int(value[4:6], 16), int(value[6:8], 16))
    raise ValueError(f"Invalid color: {hex_code}")

# <META - ROLE : Draw outer frame layer with rounded rectangle | L35-40>
def _draw_outer(draw: ImageDraw.ImageDraw, geometry: dict[str, Any], colors: dict[str, Any], layers: list[str]) -> None:
    if "outer_frame" not in layers:
        return
    spec = geometry["outer_frame"]
    bbox = [(spec["bbox"][0], spec["bbox"][1]), (spec["bbox"][2], spec["bbox"][3])]
    draw.rounded_rectangle(bbox, radius=int(spec["corner_radius"]), fill=_rgba(colors["outer_frame_bg_hex"]), outline=_rgba(colors["outer_frame_border_hex"]), width=int(spec["border_width"]))

# <META - ROLE : Draw socket well layer with rounded rectangle | L43-48>
def _draw_socket(draw: ImageDraw.ImageDraw, geometry: dict[str, Any], colors: dict[str, Any], layers: list[str]) -> None:
    if "socket_well" not in layers:
        return
    spec = geometry["socket_well"]
    bbox = [(spec["bbox"][0], spec["bbox"][1]), (spec["bbox"][2], spec["bbox"][3])]
    draw.rounded_rectangle(bbox, radius=int(spec["corner_radius"]), fill=_rgba(colors["socket_well_bg_hex"]))

# <META - ROLE : Draw inner keycap layers for normal, pressed, hover, and disabled states | L51-63>
def _draw_inner(draw: ImageDraw.ImageDraw, geometry: dict[str, Any], colors: dict[str, Any], layers: list[str]) -> None:
    state_by_layer = {
        "inner_keycap_normal": ("normal_bbox", "keycap_body_normal_hex", "keycap_border_normal_hex"),
        "inner_keycap_pressed": ("pressed_bbox", "keycap_body_pressed_hex", "keycap_border_pressed_hex"),
        "inner_keycap_hover": ("normal_bbox", "keycap_body_hover_hex", "keycap_border_hover_hex"),
        "inner_keycap_disabled": ("normal_bbox", "keycap_body_disabled_hex", "keycap_border_disabled_hex"),
    }
    spec = geometry["inner_keycap"]
    for layer, (bbox_name, body_color, border_color) in state_by_layer.items():
        if layer in layers:
            bbox_value = spec[bbox_name]
            bbox = [(bbox_value[0], bbox_value[1]), (bbox_value[2], bbox_value[3])]
            draw.rounded_rectangle(bbox, radius=int(spec["corner_radius"]), fill=_rgba(colors[body_color]), outline=_rgba(colors[border_color]), width=int(spec["border_width"]))

# <META - ROLE : Draw visor window layer with rounded rectangle | L66-73>
def _draw_visor(draw: ImageDraw.ImageDraw, geometry: dict[str, Any], colors: dict[str, Any], layers: list[str]) -> None:
    bbox_name = "pressed_bbox" if "visor_window_pressed" in layers else "normal_bbox"
    if "visor_window_pressed" not in layers and "visor_window_normal" not in layers:
        return
    spec = geometry["visor_window"]
    bbox_value = spec[bbox_name]
    bbox = [(bbox_value[0], bbox_value[1]), (bbox_value[2], bbox_value[3])]
    draw.rounded_rectangle(bbox, radius=int(spec["corner_radius"]), fill=_rgba(colors["visor_bg_hex"]), outline=_rgba(colors["visor_border_hex"]), width=int(spec["border_width"]))

# <META - ROLE : Draw all keycap layers: outer, socket, inner, and visor | L76-81>
def _draw_keycap_layers(image: Image.Image, geometry: dict[str, Any], colors: dict[str, Any], layers: list[str]) -> None:
    draw = ImageDraw.Draw(image)
    _draw_outer(draw, geometry, colors, layers)
    _draw_socket(draw, geometry, colors, layers)
    _draw_inner(draw, geometry, colors, layers)
    _draw_visor(draw, geometry, colors, layers)

# <META - ROLE : Bake fused 9-slice assets from spec and geometry | L84-93>
def _bake_fused_9slices(spec: dict[str, Any], geometry: dict[str, Any], colors: dict[str, Any]) -> list[Path]:
    written: list[Path] = []
    for asset_id, asset_spec in spec["nine_patch"].items():
        image = Image.new("RGBA", (int(asset_spec["width"]), int(asset_spec["height"])), (0, 0, 0, 0))
        _draw_keycap_layers(image, geometry, colors, asset_spec["layers"])
        output = OUTPUT_DIR / f"{asset_id}_9slice.png"
        image.save(output)
        written.append(output)
        print(f"[SUCCESS] 9-Slice 베이킹 완료 -> {output} ({image.width}x{image.height})")
    return written

# <META - ROLE : Copy generated assets to core shared directory | L96-100>
def _save_synced_assets(paths: list[Path]) -> None:
    CORE_DIR.mkdir(parents=True, exist_ok=True)
    for path in paths:
        shutil.copy2(path, CORE_DIR / path.name)
    print(f"[SUCCESS] 코어 공용 에셋 동기화 완료 -> {CORE_DIR}")

# <META - ROLE : Bake pantograph keycap atlas and sync core assets | L103-118>
def bake_atlas() -> None:
    if not JSON_PATH.exists():
        raise FileNotFoundError(f"[ERROR] SSOT JSON 파일 없음: {JSON_PATH}")
    spec: dict[str, Any] = json.loads(JSON_PATH.read_text(encoding="utf-8"))
    canvas = spec["canvas"]
    geometry = spec["geometry"]
    colors = spec["colors"]
    atlas = Image.new("RGBA", (int(canvas["total_width"]), int(canvas["total_height"])), (0, 0, 0, 0))
    for slot in spec["slots"]:
        slot_image = Image.new("RGBA", (int(canvas["slot_width"]), int(canvas["slot_height"])), (0, 0, 0, 0))
        _draw_keycap_layers(slot_image, geometry, colors, slot["layers"])
        atlas.paste(slot_image, (int(slot["index"]) * int(canvas["slot_width"]), 0), slot_image)
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    atlas.save(OUTPUT_FILE)
    print(f"[SUCCESS] 클린 팬터그래프 키캡 아틀라스 베이킹 완료 -> {OUTPUT_FILE} ({atlas.width}x{atlas.height})")
    _save_synced_assets([OUTPUT_FILE] + _bake_fused_9slices(spec, geometry, colors))

# <META - ROLE : CLI entrypoint for pantograph keycap atlas generation | L121-132>
def main() -> int:
    global JSON_PATH, OUTPUT_DIR, OUTPUT_FILE
    parser = argparse.ArgumentParser(description="Pantograph keycap atlas generator")
    parser.add_argument("--output", type=Path, default=OUTPUT_FILE)
    parser.add_argument("--profile", type=Path, default=JSON_PATH)
    parser.add_argument("--preview", action="store_true", help="Compatibility flag")
    args = parser.parse_args()
    JSON_PATH = args.profile
    OUTPUT_FILE = args.output
    OUTPUT_DIR = args.output.parent
    bake_atlas()
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
