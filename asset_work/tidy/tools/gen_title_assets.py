#!/usr/bin/env python3
# <META - FILE SUMMARY - Bake title screen assets: keycaps, text, sword, and button>
"""
tools/assets/generators/gen_title_assets.py
게임 내 키캡 SSOT(base_keycap_1u.json) 스타일 기반 2.5D 입체 키캡 및
손잡이/눈 상단 이동 + 검 날 부분에 [ l ] 텍스트 완벽 수평 배치 에셋 베이킹 엔진:
1. Y, W 키캡 (256x256): 4방향 스커트 음영(상/좌/우/하) + 4개 코너 사선 베벨 라인 + 오목 웰 + monogram 폰트 각인
2. 사이버네틱 대검 [ L ] (280x560):
   - 손잡이 & 바이저 눈: 글자 라인 위(상단 y: 20~190)로 이동!
   - [ l ] 텍스트: 검의 날(도신) 부분(y: 200~340, 중심 cy=270)에 위치하여 or, d와 완벽한 수평선 일치!
   - 검 날 끝단: 하단(y: 340~540)으로 뾰족하게 연장
3. 레트로 디지털 텍스트 (our, or, d): 240px 대형 폰트
4. 메뉴 버튼 (500x100)
"""

import argparse
from pathlib import Path
import sys

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:
    print("[ERROR] Pillow 필요: pip install Pillow")
    sys.exit(1)

TITLE_DIR = Path("features/ui/title")
FONT_PATH = Path("core/asset/font/monogram.ttf")

# <META - ROLE : Convert hex color code to RGBA tuple | L30-32>
def _rgba(hex_code: str, alpha: int = 255) -> tuple[int, int, int, int]:
    h = hex_code.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), alpha)

# <META - ROLE : Draw text with circular outline stroke | L35-49>
def _draw_outlined_text(
    d: ImageDraw.ImageDraw,
    pos: tuple[int, int],
    text: str,
    font: ImageFont.FreeTypeFont,
    fill_col: tuple[int, int, int, int],
    stroke_col: tuple[int, int, int, int],
    stroke_width: int = 8
) -> None:
    x, y = pos
    for ox in range(-stroke_width, stroke_width + 1):
        for oy in range(-stroke_width, stroke_width + 1):
            if ox * ox + oy * oy <= stroke_width * stroke_width:
                d.text((x + ox, y + oy), text, font=font, fill=stroke_col)
    d.text((x, y), text, font=font, fill=fill_col)

# <META - ROLE : Bake 2.5D mechanical keycap with skirt shading and bevel | L52-111>
def bake_keycap_2_5d(letter: str, pressed: bool = False) -> Image.Image:
    """
    게임 내 키캡 SSOT (base_keycap_1u.json) 완벽 일치 2.5D 기계식 키캡 (256x256):
    - 4방향 스커트 폴리곤 음영: 상단(하이라이트), 좌측(측면광), 우측(음영), 하단(깊은 그림자)
    - 4개 코너 사선 베벨 라인
    - 상판 오목 웰(Well) 바디 & 미세 하이라이트
    - monogram.ttf 각인
    """
    w, h = 256, 256
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    dy = 12 if pressed else 0

    bx0, by0 = 24, 20 + dy
    bx1, by1 = 232, 236 + dy

    tx0, ty0 = 64, 44 + dy
    tx1, ty1 = 216, 184 + dy

    top_skirt_fill = _rgba("#E2E8F0") if pressed else _rgba("#F8FAFC")
    d.polygon([(bx0, by0), (bx1, by0), (tx1, ty0), (tx0, ty0)], fill=top_skirt_fill)

    left_skirt_fill = _rgba("#94A3B8") if pressed else _rgba("#CBD5E1")
    d.polygon([(bx0, by0), (tx0, ty0), (tx0, ty1), (bx0, by1)], fill=left_skirt_fill)

    right_skirt_fill = _rgba("#64748B") if pressed else _rgba("#94A3B8")
    d.polygon([(bx1, by0), (bx1, by1), (tx1, ty1), (tx1, ty0)], fill=right_skirt_fill)

    bottom_skirt_fill = _rgba("#334155") if pressed else _rgba("#64748B")
    d.polygon([(bx0, by1), (tx0, ty1), (tx1, ty1), (bx1, by1)], fill=bottom_skirt_fill)

    bevel_col = _rgba("#0F172A")
    bw = 4
    d.line([(bx0, by0), (tx0, ty0)], fill=bevel_col, width=bw)
    d.line([(bx1, by0), (tx1, ty0)], fill=bevel_col, width=bw)
    d.line([(bx0, by1), (tx0, ty1)], fill=bevel_col, width=bw)
    d.line([(bx1, by1), (tx1, ty1)], fill=bevel_col, width=bw)

    d.line([(bx0, by0), (bx1, by0), (bx1, by1), (bx0, by1), (bx0, by0)], fill=bevel_col, width=bw)

    cap_top_fill = _rgba("#E2E8F0") if pressed else _rgba("#FFFFFF")
    d.rounded_rectangle([(tx0, ty0), (tx1, ty1)], radius=16, fill=cap_top_fill, outline=bevel_col, width=bw)

    dish_fill = _rgba("#CBD5E1") if pressed else _rgba("#F1F5F9")
    d.rounded_rectangle([(tx0 + 8, ty0 + 8), (tx1 - 8, ty1 - 8)], radius=12, fill=dish_fill, outline=_rgba("#E2E8F0"), width=2)

    if FONT_PATH.exists():
        font = ImageFont.truetype(str(FONT_PATH), 170)
        bbox = font.getbbox(letter)
        text_w = bbox[2] - bbox[0]
        text_h = bbox[3] - bbox[1]
        cx = (tx0 + tx1 - text_w) // 2 - bbox[0]
        cy = (ty0 + ty1 - text_h) // 2 - bbox[1]
        glyph_col = _rgba("#090D16")
        for ox in [-2, -1, 0, 1, 2]:
            for oy in [-2, -1, 0, 1, 2]:
                d.text((cx + ox, cy + oy), letter, font=font, fill=glyph_col)

    return img

# <META - ROLE : Bake retro digital text with unified baseline | L114-145>
def bake_retro_text(text: str) -> Image.Image:
    """
    monogram.ttf 240px 레트로 디지털 텍스트:
    - 핵심: 모든 텍스트 캔버스의 높이를 200px로 통일하고,
    - 베이스라인을 y = 145 에 정확히 고정하여 or, d, our 간의 수평선을 완벽히 일치시킴!
    """
    font_size = 240
    font = ImageFont.truetype(str(FONT_PATH), font_size)
    bbox = font.getbbox(text)
    text_w = bbox[2] - bbox[0]

    pad_x = 24
    w = text_w + pad_x * 2
    h = 200

    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    pos_x = pad_x - bbox[0]
    pos_y = 145 - 160

    _draw_outlined_text(
        d=d,
        pos=(pos_x, pos_y),
        text=text,
        font=font,
        fill_col=_rgba("#FFFFFF"),
        stroke_col=_rgba("#000000"),
        stroke_width=9
    )

    return img

# <META - ROLE : Bake cybernetic sword with aligned bracket text engraving | L148-224>
def bake_cybernetic_sword_aligned() -> tuple[Image.Image, Image.Image]:
    """
    사이버네틱 대검 [ L ] (280x560):
    - Godot의 Row2 (높이 480)에서 TextOr, TextD의 높이가 200px일 때
      Row2 로컬 좌표계에서 베이스라인은 140 + 145 = 285px.
    - 대검 텍스처(560px) 내에서 베이스라인 = 285 * 560 / 480 = 332.5px!
    - 따라서:
      1. 손잡이 & 폼멜: 상단 위쪽 (y: 16 ~ 100)
      2. 크로스가드: (y: 100 ~ 132)
      3. 바이저 & 디지털 눈: 글자 라인 위쪽 (y: 132 ~ 194)
      4. 검의 날(도신): (y: 194 ~ 546)
      5. [ l ] 글자: 검의 날(도신) 부분에 완벽히 새겨짐!
         베이스라인 332px (pos_y = 332 - 160 = 172) -> or, d와 오차 0.0px 수평 일치!
    """
    w, h = 280, 560
    body = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    db = ImageDraw.Draw(body)

    cx = w // 2

    db.rectangle([(cx - 24, 16), (cx + 24, 30)], fill=_rgba("#E2E8F0"), outline=_rgba("#0F172A"), width=4)
    db.rectangle([(cx - 16, 30), (cx + 16, 38)], fill=_rgba("#94A3B8"), outline=_rgba("#0F172A"), width=3)

    for gy in range(38, 98, 12):
        db.rectangle([(cx - 14, gy), (cx + 14, gy + 9)], fill=_rgba("#F8FAFC"), outline=_rgba("#0F172A"), width=3)
        db.line([(cx - 12, gy + 4), (cx + 12, gy + 4)], fill=_rgba("#64748B"), width=2)

    guard_poly = [
        (cx - 56, 100), (cx + 56, 100),
        (cx + 64, 118), (cx + 44, 130),
        (cx - 44, 130), (cx - 64, 118)
    ]
    db.polygon(guard_poly, fill=_rgba("#E2E8F0"), outline=_rgba("#0F172A"))
    db.line(guard_poly + [guard_poly[0]], fill=_rgba("#0F172A"), width=4)

    vy0, vy1 = 132, 190
    db.rounded_rectangle([(cx - 48, vy0 - 3), (cx + 48, vy1 + 3)], radius=14, fill=_rgba("#0B0F19"), outline=_rgba("#000000"), width=4)
    db.rounded_rectangle([(cx - 42, vy0), (cx + 42, vy1)], radius=12, fill=_rgba("#071526"), outline=_rgba("#0284C7"), width=3)
    db.line([(cx - 34, vy0 + 4), (cx + 34, vy0 + 4)], fill=_rgba("#38BDF8", 160), width=2)

    blade_poly = [
        (cx - 46, vy1 + 3), (cx + 46, vy1 + 3),
        (cx + 36, 480), (cx, 546),
        (cx - 36, 480)
    ]
    db.polygon(blade_poly, fill=_rgba("#F8FAFC"), outline=_rgba("#0F172A"))
    db.line(blade_poly + [blade_poly[0]], fill=_rgba("#0F172A"), width=5)

    db.line([(cx, vy1 + 5), (cx, 510)], fill=_rgba("#94A3B8"), width=4)
    db.polygon([(cx - 44, vy1 + 5), (cx, vy1 + 5), (cx, 510), (cx - 34, 478)], fill=_rgba("#FFFFFF", 150))
    db.polygon([(cx, vy1 + 5), (cx + 44, vy1 + 5), (cx + 34, 478), (cx, 510)], fill=_rgba("#CBD5E1", 150))

    if FONT_PATH.exists():
        font_240 = ImageFont.truetype(str(FONT_PATH), 240)
        ty = 172

        _draw_outlined_text(db, (12, ty), "[", font_240, _rgba("#00E5FF"), _rgba("#000000"), 8)

        _draw_outlined_text(db, (cx - 45, ty), "l", font_240, _rgba("#FFFFFF"), _rgba("#000000"), 9)

        _draw_outlined_text(db, (w - 102, ty), "]", font_240, _rgba("#00E5FF"), _rgba("#000000"), 8)

    eye_img = Image.new("RGBA", (54, 42), (0, 0, 0, 0))
    de = ImageDraw.Draw(eye_img)
    ecx, ecy = 27, 21

    cyan_glow = _rgba("#00E5FF")
    cyan_core = _rgba("#FFFFFF")

    de.rounded_rectangle([(ecx - 15, ecy - 12), (ecx + 15, ecy + 12)], radius=7, fill=_rgba("#00E5FF", 50), outline=cyan_glow, width=3)
    de.rectangle([(ecx - 5, ecy - 5), (ecx + 5, ecy + 5)], fill=cyan_core)
    de.rectangle([(ecx - 2, ecy - 11), (ecx + 2, ecy - 8)], fill=cyan_glow)
    de.rectangle([(ecx - 2, ecy + 8), (ecx + 2, ecy + 11)], fill=cyan_glow)
    de.rectangle([(ecx - 20, ecy - 2), (ecx - 17, ecy + 2)], fill=cyan_glow)
    de.rectangle([(ecx + 17, ecy - 2), (ecx + 20, ecy + 2)], fill=cyan_glow)

    return body, eye_img

# <META - ROLE : Bake menu button panel with rounded borders | L227-237>
def bake_menu_btn() -> Image.Image:
    """메뉴 버튼 패널 (500x100)"""
    w, h = 500, 100
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    d.rounded_rectangle([(8, 8), (w - 8, h - 8)], radius=20, fill=_rgba("#0F172A"), outline=_rgba("#000000"), width=5)
    d.rounded_rectangle([(13, 10), (w - 13, h - 14)], radius=17, fill=_rgba("#FFFFFF"), outline=_rgba("#0F172A"), width=4)
    d.rounded_rectangle([(20, 16), (w - 20, h - 20)], radius=12, outline=_rgba("#E2E8F0"), width=2)

    return img

# <META - ROLE : CLI entrypoint for title asset baking | L240-279>
def main() -> None:
    parser = argparse.ArgumentParser(description="Generate title screen assets")
    parser.add_argument(
        "--output",
        type=Path,
        default=TITLE_DIR,
        help="title feature root directory",
    )
    title_dir: Path = parser.parse_args().output
    logo_assets_dir: Path = title_dir / "logo" / "assets"
    logo_legacy_dir: Path = logo_assets_dir / "legacy"
    button_assets_dir: Path = title_dir / "buttons" / "assets"
    diorama_assets_dir: Path = title_dir / "diorama" / "assets"
    diorama_legacy_dir: Path = diorama_assets_dir / "legacy"
    for asset_dir in [
        logo_assets_dir,
        logo_legacy_dir,
        button_assets_dir,
        diorama_assets_dir,
        diorama_legacy_dir,
    ]:
        asset_dir.mkdir(parents=True, exist_ok=True)
    print(f"[GEN] title feature asset slices -> {title_dir}")

    bake_keycap_2_5d("Y", pressed=False).save(logo_legacy_dir / "title_keycap_y.png")
    bake_keycap_2_5d("Y", pressed=True).save(logo_legacy_dir / "title_keycap_y_pressed.png")
    bake_keycap_2_5d("W", pressed=False).save(logo_legacy_dir / "title_keycap_w.png")
    bake_keycap_2_5d("W", pressed=True).save(logo_legacy_dir / "title_keycap_w_pressed.png")

    bake_retro_text("our").save(logo_assets_dir / "title_text_our.png")
    bake_retro_text("or").save(logo_assets_dir / "title_text_or.png")
    bake_retro_text("d").save(logo_assets_dir / "title_text_d.png")

    sword_body, sword_eye = bake_cybernetic_sword_aligned()
    sword_body.save(diorama_assets_dir / "title_sword_body.png")
    sword_eye.save(diorama_legacy_dir / "title_sword_eye.png")

    bake_menu_btn().save(button_assets_dir / "title_menu_btn.png")

    print("[GEN] title feature asset baking complete!")

if __name__ == "__main__":
    main()
