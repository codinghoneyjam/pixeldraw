#!/usr/bin/env python3
# <META - FILE SUMMARY - Bake 128x128 keycap assets from visual profile JSON>
"""
tools/assets/generators/gen_keycap_assets.py
keycap_visual_profiles.json(순수 디자인 SSOT) 기반 키캡 128x128 에셋 베이킹 엔진
- 두꺼운 7세그먼트 직선형 각인 (width=8)
- 왜곡 없는 깔끔한 2.5D 탑다운 기계식 키캡 조형
- 누른 상태 / 안 누른 상태 / 비활성 빈 소켓 완벽 분리
"""

import argparse
import json
from pathlib import Path
import sys

try:
    from PIL import Image, ImageDraw
except ImportError:
    print("[ERROR] Pillow 필요: pip install Pillow")
    sys.exit(1)

JSON_PATH = Path("features/world/object/keycap/data/keycap_visual_profiles.json")
OUTPUT_DIR = Path("features/world/object/keycap/assets/images/keycaps")

# <META - ROLE : Convert hex color code to RGBA tuple | L26-28>
def _rgba(hex_code: str, alpha: int = 255) -> tuple[int, int, int, int]:
    h = hex_code.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), alpha)

# <META - ROLE : Render empty mechanical switch socket with stem | L31-52>
def bake_disabled_socket() -> Image.Image:
    """비활성 상태: 키캡이 완전히 뽑힌 빈 기계식 스위치 소켓"""
    img = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    d.rectangle([(16, 16), (112, 112)], fill=_rgba("#121820"), outline=_rgba("#334155"), width=3)
    d.rectangle([(24, 24), (104, 104)], fill=_rgba("#090D12"), outline=_rgba("#1E293B"), width=2)

    for bx, by in [(22, 22), (106, 22), (22, 106), (106, 106)]:
        d.rectangle([(bx - 2, by - 2), (bx + 2, by + 2)], fill=_rgba("#475569"))

    d.rectangle([(42, 42), (86, 86)], fill=_rgba("#151C26"), outline=_rgba("#2D3748"), width=2)

    stem_col = _rgba("#FF3C5A")
    stem_shadow = _rgba("#B91C1C")

    d.rectangle([(48, 60), (80, 68)], fill=stem_col, outline=stem_shadow, width=1)

    d.rectangle([(60, 48), (68, 80)], fill=stem_col, outline=stem_shadow, width=1)

    d.rectangle([(59, 59), (69, 69)], fill=_rgba("#FF6B81"))
    return img

# <META - ROLE : Render keycap body with 2.5D bevel for pressed or unpressed state | L55-84>
def bake_cap_body(pressed: bool) -> Image.Image:
    """안 누른 상태(돌출) vs 누른 상태(하강 밀착) 키캡 바디"""
    img = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    d.rectangle([(16, 16), (112, 112)], fill=_rgba("#0B1017"), outline=_rgba("#334155"), width=3)

    if not pressed:

        bx0, by0, bx1, by1 = 20, 20, 108, 108
        tx0, ty0, tx1, ty1 = 28, 22, 100, 88

        d.polygon([(bx0, by0), (bx1, by0), (tx1, ty0), (tx0, ty0)], fill=_rgba("#2A384C"))
        d.polygon([(bx0, by0), (tx0, ty0), (tx0, ty1), (bx0, by1)], fill=_rgba("#1B2432"))
        d.polygon([(bx1, by0), (bx1, by1), (tx1, ty1), (tx1, ty0)], fill=_rgba("#151D29"))
        d.polygon([(bx0, by1), (tx0, ty1), (tx1, ty1), (bx1, by1)], fill=_rgba("#0E141E"))

        d.rectangle([(tx0, ty0), (tx1, ty1)], fill=_rgba("#1E293B"), outline=_rgba("#38BDF8"), width=3)

        d.rectangle([(tx0 + 3, ty0 + 3), (tx1 - 3, ty1 - 3)], outline=_rgba("#0F172A"), width=1)
    else:

        tx0, ty0, tx1, ty1 = 28, 30, 100, 96

        d.rectangle([(22, 24), (106, 102)], fill=_rgba("#101824"))

        d.rectangle([(tx0, ty0), (tx1, ty1)], fill=_rgba("#334155"), outline=_rgba("#FBBF24"), width=3)
        d.rectangle([(tx0 + 3, ty0 + 3), (tx1 - 3, ty1 - 3)], outline=_rgba("#1E293B"), width=1)

    return img

# <META - ROLE : Draw thick 7-segment straight-line digit on draw context | L87-113>
def draw_thick_straight_digit(d: ImageDraw.ImageDraw, digit: int, x: int, y: int, w: int, h: int, color: tuple) -> None:
    """두꺼운(width=8) 직선 세그먼트 선분 제도"""
    mid_y = y + (h // 2)

    seg = {
        'a': [(x, y), (x + w, y)],
        'b': [(x + w, y), (x + w, mid_y)],
        'c': [(x + w, mid_y), (x + w, y + h)],
        'd': [(x, y + h), (x + w, y + h)],
        'e': [(x, mid_y), (x, y + h)],
        'f': [(x, y), (x, mid_y)],
        'g': [(x, mid_y), (x + w, mid_y)]
    }
    table = {
        0: ['a', 'b', 'c', 'd', 'e', 'f'],
        1: ['b', 'c'],
        2: ['a', 'b', 'g', 'e', 'd'],
        3: ['a', 'b', 'g', 'c', 'd'],
        4: ['f', 'g', 'b', 'c'],
        5: ['a', 'f', 'g', 'c', 'd'],
        6: ['a', 'f', 'g', 'e', 'c', 'd'],
        7: ['a', 'b', 'c'],
        8: ['a', 'b', 'c', 'd', 'e', 'f', 'g'],
        9: ['a', 'b', 'c', 'd', 'f', 'g'],
    }
    for s in table.get(digit, []):
        d.line(seg[s], fill=color, width=8, joint="curve")

# <META - ROLE : Render two-digit legend asset with thick straight-line engraving | L116-128>
def bake_legend(zid: int, theme_hex: str) -> Image.Image:
    """두껍고 선명한 01~08 직선 각인 에셋 (상판 중심 기준)"""
    img = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    col = _rgba(theme_hex)

    w, h = 22, 40
    d1 = zid // 10
    d2 = zid % 10

    draw_thick_straight_digit(d, d1, 35, 35, w, h, col)
    draw_thick_straight_digit(d, d2, 70, 35, w, h, col)
    return img

# <META - ROLE : Load visual profiles and bake all keycap image assets | L131-147>
def run_pipeline(output_dir: Path = OUTPUT_DIR) -> None:
    with open(JSON_PATH, "r", encoding="utf-8") as f:
        spec = json.load(f)

    themes = spec.get("theme_neon_hues", {})
    output_dir.mkdir(parents=True, exist_ok=True)

    bake_disabled_socket().save(output_dir / "keycap_disabled_socket.png")
    bake_cap_body(False).save(output_dir / "keycap_unpressed.png")
    bake_cap_body(True).save(output_dir / "keycap_pressed.png")

    for zid in range(1, 9):
        key_str = f"{zid:02d}"
        hex_col = themes.get(key_str, "#38BDF8")
        bake_legend(zid, hex_col).save(output_dir / f"legend_{zid:02d}.png")

    print(f"[SUCCESS] 고대비/두꺼운 각인 키캡 에셋 베이킹 완료 -> {output_dir}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Generate world keycap assets")
    parser.add_argument("--output", type=Path, default=OUTPUT_DIR, help="output directory")
    run_pipeline(parser.parse_args().output)
