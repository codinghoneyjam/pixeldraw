#!/usr/bin/env python3
# <META - FILE SUMMARY - Generate 128px HUD icon atlas, emissive mask, and manifest from shape JSON>
"""128px HUD 아이콘 아틀라스 생성기 (refer.png 및 인게임 스프라이트 화풍 완벽 일치).

refer.png 및 게임 내 적(keybot)/플레이어(mouse_hero)의 Bold & Chunky 사이버네틱 픽셀
화풍에 맞추어, 128x128 픽셀 캔버스에 직접 선과 면을 제도하여 아틀라스를 베이킹한다.
- 통일된 검은색 외곽선(width=3)
- 맑고 쨍한 원색 네온 컬러(크림슨 레드, 네온 시안, 일렉트릭 옐로우, 오렌지, 마젠타)
- 한눈에 직관적으로 인지되는 심볼릭 조형미(통통한 하트, 챔퍼 방패, 번개, 열린 책, 원형 시계 등)

산출물:
  features/ui/common/assets/generated/atlases/hud_icons_128_atlas.png (512x512 RGBA, 4x4 그리드)
  features/ui/common/assets/generated/atlases/hud_icons_128_mask.png  (네온 발광 emissive 마스크)
  features/ui/stage/asset/recipe/hud_icons_128.json                (각 아이콘 Rect2 매핑)

Usage:
  python -m dev.tools.assets.generators.gen_hud_icons_128
"""

from __future__ import annotations

import argparse
import json
import math
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

from dev.tools.assets.core.geometry import load_shape_file, render_shape_layer
from dev.tools.assets.core.masks import build_emissive_mask
from dev.tools.assets.core.paths import (
    ATLAS_UI,
    ATLAS_UI_WRITE,
    MANIFEST_UI,
    SHAPES_HUD_ICONS,
    dst,
    res,
    src,
)

ATLAS_DIR: Path = dst(ATLAS_UI_WRITE)
MANIFEST_DIR: Path = dst(MANIFEST_UI)
SHAPES_DIR: Path = src(SHAPES_HUD_ICONS)
ATLAS_PATH: Path = ATLAS_DIR / "hud_icons_128_atlas.png"
MASK_PATH: Path = ATLAS_DIR / "hud_icons_128_mask.png"
MANIFEST_PATH: Path = MANIFEST_DIR / "hud_icons_128.json"

CELL: int = 128
COLUMNS: int = 4
ROWS: int = 4

ICON_NAMES: list[str] = [
    "heart", "shield", "lightning", "speaker",
    "book", "gear", "stopwatch", "diamond_gem",
    "cross_mount", "kill_skull", "chest",
]

# <META - ROLE : Bake the atlas + emissive neon mask from shape JSON files and return icon->Rect2 mapping. | L68-90>
def bake() -> dict[str, list[int]]:
    atlas: Image.Image = Image.new("RGBA", (CELL * COLUMNS, CELL * ROWS), (0, 0, 0, 0))
    mapping: dict[str, list[int]] = {}

    for index, name in enumerate(ICON_NAMES):
        json_path = SHAPES_DIR / f"{name}.json"
        shape_spec = load_shape_file(json_path)
        if shape_spec is None:
            raise FileNotFoundError(f"[ERROR] Icon shape JSON not found: {json_path}")

        slot_img = render_shape_layer(shape_spec, "body", {}, size=(CELL, CELL))

        ox: int = (index % COLUMNS) * CELL
        oy: int = (index // COLUMNS) * CELL
        atlas.paste(slot_img, (ox, oy), slot_img)
        mapping[name] = [ox, oy, CELL, CELL]

    ATLAS_DIR.mkdir(parents=True, exist_ok=True)
    atlas.save(ATLAS_PATH, "PNG")
    build_emissive_mask(atlas).convert("RGBA").save(MASK_PATH, "PNG")
    print(f"[SUCCESS] 아틀라스 베이킹 -> {ATLAS_PATH} ({atlas.width}x{atlas.height})")
    print(f"[SUCCESS] 네온 발광 마스크 -> {MASK_PATH}")
    return mapping

# <META - ROLE : Write the icon manifest JSON with per-icon Rect2 metadata. | L93-105>
def write_manifest(mapping: dict[str, list[int]]) -> None:
    payload: dict = {
        "atlas": res(f"{ATLAS_UI}/hud_icons_128_atlas.png"),
        "mask": res(f"{ATLAS_UI}/hud_icons_128_mask.png"),
        "cell_size": CELL,
        "columns": COLUMNS,
        "rows": ROWS,
        "gutter": 0,
        "icons": {name: {"rect": rect} for name, rect in mapping.items()},
    }
    MANIFEST_DIR.mkdir(parents=True, exist_ok=True)
    MANIFEST_PATH.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"[SUCCESS] 매니페스트 -> {MANIFEST_PATH} ({len(mapping)}종 아이콘)")

# <META - ROLE : CLI entry point that bakes assets and writes the manifest. | L108-113>
def main() -> int:
    parser = argparse.ArgumentParser(description="128px HUD 아이콘 아틀라스 생성기 (선 드로잉)")
    parser.add_argument("--preview", action="store_true", help="호환 플래그")
    parser.parse_args()
    write_manifest(bake())
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
