#!/usr/bin/env python3
# <META - FILE SUMMARY - Procedural enemy sprite atlas generator from JSON geometry specs>
"""
Enemy PNG Asset Generator
tools/assets/generators/gen_enemy_assets.py

각 적 레이어(shadow, body, visor, glow, face_*)를 128x128 RGBA PNG로 절차적으로 생성합니다.
features/entity/enemy/data/shapes/*.json 지오메트리 SSOT를 100% 참조하여 베이킹합니다.

Output: features/entity/enemy/assets/textures/enemy_keybot_<id>_sheet.png  (2048×128 아틀라스, 16슬롯)
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

try:
    from PIL import Image, ImageFilter, ImageDraw
except ImportError:
    print("[ERROR] Pillow 미설치. 실행: pip install Pillow")
    sys.exit(1)

from dev.tools.assets.core.drawing import CANVAS, _ebb, _new_canvas  # noqa: E402
from dev.tools.assets.core.geometry import load_shape_file, render_shape_layer  # noqa: E402
from dev.tools.assets.core.paths import SHAPES_ENEMY, src  # noqa: E402
from dev.tools.data.config import PROJECT_ROOT  # noqa: E402
from dev.tools.data.database import build_database, query_database  # noqa: E402

FACE_STATES: list[str] = ["neutral", "alert", "damaged", "dead"]
SLOT_ORDER: list[str] = [
    "shadow", "body", "glyph", "scratch",
    "visor", "visor_cracked",
    "face_neutral", "face_alert", "face_damaged", "face_dead",
]
SHAPES_DIR: Path = src(SHAPES_ENEMY)

SHAPE_IDS: list[str] = ["C-001", "R-003", "T-002", "B-001"]
SHAPE_SOURCE_OVERRIDE: dict[str, str] = {"B-001": "C-001"}

DEFAULT_SPECS: dict[str, dict] = {
    "C-001": {
        "body_color": "#F4B41A", "visor_color": "#000000",
        "eye_color": "#FFE84D", "glow_color": "#FFD700",
        "shadow_rx": 28, "shadow_ry": 6, "shadow_cy": 110,
    },
    "T-002": {
        "body_color": "#E52B50", "visor_color": "#000000",
        "eye_color": "#FF3355", "glow_color": "#FF1830",
        "shadow_rx": 30, "shadow_ry": 5, "shadow_cy": 110,
    },
    "R-003": {
        "body_color": "#22325E", "visor_color": "#000000",
        "eye_color": "#59A6FF", "glow_color": "#3388FF",
        "shadow_rx": 28, "shadow_ry": 6, "shadow_cy": 110,
    },
}

_CACHED_SHAPES: dict[str, dict] = {}

# <META - ROLE : Load and cache a shape JSON spec by enemy ID | L65-76>
def _get_shape(enemy_id: str) -> dict:
    """선언적 벡터 도형 JSON 명세 반환 (캐싱 지원)."""
    if enemy_id in _CACHED_SHAPES:
        return _CACHED_SHAPES[enemy_id]
    source_id = SHAPE_SOURCE_OVERRIDE.get(enemy_id, enemy_id)
    shape_path = SHAPES_DIR / f"{source_id}.json"
    if shape_path.exists():
        loaded = load_shape_file(shape_path)
        if loaded:
            _CACHED_SHAPES[enemy_id] = loaded
            return loaded
    raise FileNotFoundError(f"[ERROR] shape JSON 미발견: {shape_path}")

# <META - ROLE : Discover enemy IDs from shape files or defaults | L79-81>
def _get_all_enemy_ids() -> list[str]:
    """반환: 명시적 화이트리스트 SHAPE_IDS."""
    return list(SHAPE_IDS)

# <META - ROLE : Load visual specs from SQLite with fallback to defaults | L84-110>
def _load_visual_specs() -> dict[str, dict]:
    """SQLite에서 시각 스펙을 로드하고 실패 시 기본값으로 복구합니다."""
    defaults = dict(DEFAULT_SPECS)
    try:
        database = build_database(PROJECT_ROOT)
        rows = query_database(database, "SELECT * FROM entity_keybot_visual_specs ORDER BY rowid")
        specs: dict[str, dict] = {}
        for row in rows:
            sid = str(row.get("shape_id", ""))
            vid = str(row.get("visual_id", ""))
            key = sid if sid in defaults else (vid if vid in defaults else "")
            if not key:
                continue
            d = defaults[key]
            specs[key] = {
                "body_color": str(row.get("body_color_hex", d["body_color"])),
                "visor_color": str(row.get("visor_color_hex", d["visor_color"])),
                "eye_color": str(row.get("eye_color_hex", d["eye_color"])),
                "glow_color": str(row.get("glow_color_hex", d["glow_color"])),
                "shadow_rx": int(row.get("shadow_rx", d["shadow_rx"])),
                "shadow_ry": int(row.get("shadow_ry", d["shadow_ry"])),
                "shadow_cy": int(row.get("shadow_cy", d["shadow_cy"])),
            }
        return specs if specs else defaults
    except Exception as error:
        print(f"[WARN] SQLite 로드 실패, 기본값 사용: {error}")
        return defaults

SPECS: dict[str, dict] = _load_visual_specs()

# <META - ROLE : Return the resolved visual spec for an enemy ID | L115-117>
def _spec(enemy_id: str) -> dict:
    """SPECS → DEFAULT_SPECS 순으로 해석된 스펙 반환."""
    return SPECS.get(enemy_id, DEFAULT_SPECS.get(enemy_id, {}))

# <META - ROLE : Render floor shadow and neon underglow ring | L120-138>
def draw_shadow(enemy_id: str) -> Image.Image:
    """하단 바닥 그림자 + 시그니처 네온 언더글로우 링 렌더링."""
    img, d = _new_canvas()
    s = _spec(enemy_id)
    cx, cy = 64, s.get("shadow_cy", 108)
    rx, ry = s.get("shadow_rx", 26), s.get("shadow_ry", 6)

    glow_hex = s.get("glow_color", "#FFD700").lstrip("#")
    if len(glow_hex) == 6:
        gr, gg, gb = int(glow_hex[0:2], 16), int(glow_hex[2:4], 16), int(glow_hex[4:6], 16)
    else:
        gr, gg, gb = (255, 215, 0)
    d.ellipse(_ebb(cx, cy, rx + 6, ry + 3), fill=(gr, gg, gb, 90))

    d.ellipse(_ebb(cx, cy, rx + 3, ry + 2), outline=(gr, gg, gb, 240), width=2)

    d.ellipse(_ebb(cx, cy, rx, ry), fill=(10, 15, 25, 210))
    d.ellipse(_ebb(cx, cy, rx - 6, ry - 2), fill=(5, 8, 14, 240))
    return img

# <META - ROLE : Dispatch layer rendering by name | L141-149>
def _load_or_draw(enemy_id: str, layer: str) -> Image.Image:
    """레이어 이름으로 드로잉 디스패치 (100% 선언적 JSON SSOT 기반)."""
    if layer == "empty":
        img, _ = _new_canvas()
        return img
    if layer == "shadow":
        return draw_shadow(enemy_id)
    shape = _get_shape(enemy_id)
    return render_shape_layer(shape, layer, _spec(enemy_id))

ENEMY_SLOTS: list[str] = [
    "shadow",
    "body_100",
    "body_50",
    "body_25",
    "visor_100",
    "visor_50",
    "visor_25",
    "visor_flipped",
    "face_idle_100",
    "face_idle_50",
    "face_idle_25",
    "face_alert",
    "face_damaged",
    "face_dead",
    "slot_keycap_body",
    "slot_keycap_pressed",
]

# <META - ROLE : Generate a 2048x128 atlas sheet with 16 enemy slots | L171-182>
def generate_sheet(enemy_id: str, out_dir: Path) -> Path:
    """16개 슬롯을 2048×128 아틀라스로 생성."""
    total_slots = len(ENEMY_SLOTS)
    sheet = Image.new("RGBA", (CANVAS * total_slots, CANVAS), (0, 0, 0, 0))
    for idx, layer_name in enumerate(ENEMY_SLOTS):
        part_img = _load_or_draw(enemy_id, layer_name)
        sheet.paste(part_img, (idx * CANVAS, 0), part_img)

    sheet_path = out_dir / ("enemy_keybot_" + enemy_id.lower().replace("-", "") + "_sheet.png")
    sheet.save(sheet_path, "PNG")
    print(f"  [ATLAS] {sheet_path} (2048×128, 16 slots full-fit)")
    return sheet_path

# <META - ROLE : Generate a composite preview image of all layers | L185-192>
def generate_composite(enemy_id: str, out_dir: Path) -> None:
    """모든 레이어를 합성한 미리보기 이미지 생성."""
    base = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    for layer in ["shadow", "body_100", "visor_100", "face_idle_100"]:
        base = Image.alpha_composite(base, _load_or_draw(enemy_id, layer))
    preview = out_dir / ("enemy_keybot_" + enemy_id.lower().replace("-", "") + "_preview.png")
    base.save(preview, "PNG")
    print(f"  [PREVIEW] {preview}")

# <META - ROLE : Generate atlas sheet and optional composite for an enemy | L195-200>
def generate_enemy(enemy_id: str, output_base: str, composite: bool = True) -> None:
    out_dir = Path(output_base)
    out_dir.mkdir(parents=True, exist_ok=True)
    generate_sheet(enemy_id, out_dir)
    if composite:
        generate_composite(enemy_id, out_dir)

# <META - ROLE : CLI entry point for enemy atlas generation | L203-223>
def main() -> None:
    parser = argparse.ArgumentParser(
        description="Enemy Atlas Sheet Generator (2048×128 RGBA, 16 slots)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    all_enemies = _get_all_enemy_ids()
    parser.add_argument("--enemy", default="all",
                        help=f"{' | '.join(all_enemies)} | all  (기본: all)")
    parser.add_argument("--output", default="features/entity/enemy/assets/textures",
                        help="출력 디렉토리 (기본: features/entity/enemy/assets/textures)")
    parser.add_argument("--composite", action="store_true", default=True,
                        help="미리보기 preview_composite.png 함께 생성 (기본: True)")
    args = parser.parse_args()

    enemies = all_enemies if args.enemy == "all" else [args.enemy]

    for eid in enemies:
        print(f"\n→ {eid} 아틀라스 시트 생성 중 (JSON SSOT)...")
        generate_enemy(eid, args.output, args.composite)

    print("\n완료.")

if __name__ == "__main__":
    main()
