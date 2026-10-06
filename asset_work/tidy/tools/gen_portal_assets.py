#!/usr/bin/env python3
# <META - FILE SUMMARY - Generate master keycap and sky warp gate textures>
"""
Master Keycap & Sky Warp Gate Asset Generator
tools/assets/generators/gen_portal_assets.py

assetdb/world/object/portal_keycap_master.json 명세를 로드하고,
주입받은 색상 프로파일(Color Injection)을 반영하여 포탈 및 TP 장치 텍스처를 자동으로 베이킹합니다.

입력은 `paths.SHAPE_PORTAL_KEYCAP`, 출력은 `paths.IMAGES_PORTAL` 상수가 SSOT다.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

try:
    from PIL import Image, ImageDraw, ImageFilter
except ImportError:
    print("[ERROR] Pillow 필요: pip install Pillow")
    sys.exit(1)

from dev.tools.assets.core import paths
from dev.tools.assets.core.geometry import load_shape_file, render_shape_layer

SHAPE_PATH: Path = paths.src(paths.SHAPE_PORTAL_KEYCAP)
DEFAULT_OUT_DIR: Path = paths.dst(paths.IMAGES_PORTAL)

DEFAULT_THEME: dict[str, str] = {
    "socket_plate": "#0E141C",
    "socket_rim": "#253448",
    "keycap_skirt_top": "#1D382B",
    "keycap_skirt_side": "#13261D",
    "keycap_skirt_shadow": "#0A1610",
    "keycap_face": "#152E22",
    "accent_neon": "#00FF88",
    "accent_glow": "#39FF14",
    "sky_gate_primary": "#00FF88",
    "sky_gate_secondary": "#39FF14",
    "core_black": "#050A07",
}

# <META - ROLE : Generate cascade beam laser texture with white core | L45-88>
def _bake_cascade_beam(size: tuple[int, int] = (256, 512), color_hex: str = "#00FF88") -> Image.Image:
    """천장에서 쏟아지는 압도적인 초록빛 레이저 기둥 텍스처 생성 (화이트 코어 + 에메랄드 플라즈마)."""
    img = Image.new("RGBA", size, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    h = color_hex.lstrip("#")
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)

    w, height = size
    mid_x = w // 2

    for y in range(height):
        progress = y / float(height)

        edge_fade = 1.0 if y < height - 60 else (height - y) / 60.0
        aura_alpha = int(120 * edge_fade)
        aura_w = int(180 + progress * 40)
        x0 = mid_x - aura_w // 2
        x1 = mid_x + aura_w // 2
        d.line([(x0, y), (x1, y)], fill=(r, g, b, aura_alpha), width=1)

        body_alpha = int(220 * edge_fade)
        body_w = int(90 + progress * 20)
        bx0 = mid_x - body_w // 2
        bx1 = mid_x + body_w // 2
        d.line([(bx0, y), (bx1, y)], fill=(r, g, b, body_alpha), width=1)

        core_alpha = int(255 * edge_fade)
        core_w = int(32 + progress * 8)
        cx0 = mid_x - core_w // 2
        cx1 = mid_x + core_w // 2
        d.line([(cx0, y), (cx1, y)], fill=(255, 255, 255, core_alpha), width=1)

    import random
    rng = random.Random(42)
    for _ in range(35):
        sx = mid_x + rng.randint(-35, 35)
        sy0 = rng.randint(0, height // 2)
        sy1 = sy0 + rng.randint(80, 240)
        sw = rng.randint(1, 3)
        d.line([(sx, sy0), (sx, min(height, sy1))], fill=(255, 255, 255, 180), width=sw)

    img = img.filter(ImageFilter.GaussianBlur(radius=3))
    return img

# <META - ROLE : Bake portal socket, keycap, sky gate, and beam textures | L91-123>
def bake_portal_assets(
    shape_path: Path = SHAPE_PATH,
    out_dir: Path = DEFAULT_OUT_DIR,
    color_overrides: dict[str, str] | None = None,
) -> None:
    """지오메트리 명세 및 주입된 색상 프로파일을 기반으로 텍스처 베이킹."""
    shape_spec = load_shape_file(shape_path)
    if not shape_spec:
        print(f"[ERROR] 지오메트리 파일을 찾을 수 없습니다: {shape_path}")
        return

    out_dir.mkdir(parents=True, exist_ok=True)

    active_theme = dict(DEFAULT_THEME)
    if color_overrides:
        active_theme.update(color_overrides)

    socket_img = render_shape_layer(shape_spec, "socket", active_theme, size=(128, 128))
    socket_img.save(out_dir / "portal_socket.png")

    unpressed_img = render_shape_layer(shape_spec, "keycap_unpressed", active_theme, size=(128, 128))
    unpressed_img.save(out_dir / "portal_keycap_unpressed.png")

    pressed_img = render_shape_layer(shape_spec, "keycap_pressed", active_theme, size=(128, 128))
    pressed_img.save(out_dir / "portal_keycap_pressed.png")

    sky_gate_img = render_shape_layer(shape_spec, "sky_gate", active_theme, size=(128, 128))
    sky_gate_img.save(out_dir / "portal_sky_gate.png")

    beam_img = _bake_cascade_beam(size=(128, 256), color_hex=active_theme.get("accent_neon", "#00FF88"))
    beam_img.save(out_dir / "portal_cascade_beam.png")

    print(f"[SUCCESS] 포탈 및 TP 장치 에셋 베이킹 완료 -> {out_dir}")

# <META - ROLE : CLI entrypoint for portal asset generation | L126-144>
def main() -> None:
    parser = argparse.ArgumentParser(description="Master Keycap Portal Asset Generator")
    parser.add_argument(
            "--output", "--out-dir", dest="output", type=Path,
            default=DEFAULT_OUT_DIR, help="output directory"
        )
    parser.add_argument("--accent-neon", type=str, default=None, help="네온 액센트 색상 (#00FF88 등)")
    parser.add_argument("--keycap-face", type=str, default=None, help="키캡 상판 색상 (#152E22 등)")
    args = parser.parse_args()

    overrides: dict[str, str] = {}
    if args.accent_neon:
        overrides["accent_neon"] = args.accent_neon
        overrides["accent_glow"] = args.accent_neon
        overrides["sky_gate_primary"] = args.accent_neon
    if args.keycap_face:
        overrides["keycap_face"] = args.keycap_face

    bake_portal_assets(out_dir=args.output, color_overrides=overrides)

if __name__ == "__main__":
    main()
