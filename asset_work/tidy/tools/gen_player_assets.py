#!/usr/bin/env python3
# <META - FILE SUMMARY - Generate player mouse hero 16-slot atlas from SSOT specs>
"""
Player Mouse Hero PNG Asset Generator (SSOT-Driven Clean 16-Slot Atlas)
tools/assets/generators/gen_player_assets.py

3대 레이어(몸통 - 바이저 - 이모티콘) 완전 분리형 아틀라스(2048x128 RGBA).
- Slot 0: Shadow
- Slot 1: Body (Chassis)
- Slot 2: Spine Stream
- Slot 3: Visor Left (3D Wrap Shell)
- Slot 4: Visor Right (3D Wrap Shell)
- Slot 5: Visor Cracked Left (Top Ridge Branch Crack)
- Slot 6: Visor Cracked Right (Side Flange Branch Crack)
- Slot 7: Face Normal (Crescent)
- Slot 8: Face Hurt (Caret Angle)
- Slot 9: Face Dead (Horizontal Bar)
- Slot 10~12: Wheel Frames 0~2
- Slot 13~15: TBD (Empty Transparent)

Output: features/entity/player/assets/textures/mouse_hero/sheet.png (2048x128)
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

sys.path.insert(0, str(Path(__file__).parent))

from dev.tools.assets.core.geometry import load_shape_file, render_layer_commands, resolve_color  # noqa: E402
from dev.tools.data.config import PROJECT_ROOT  # noqa: E402
from dev.tools.data.database import build_database, query_database  # noqa: E402

CANVAS: int = 128
TOTAL_SLOTS: int = 16
ATLAS_WIDTH: int = CANVAS * TOTAL_SLOTS
ATLAS_HEIGHT: int = CANVAS

SHAPES_DIR: Path = Path("features/entity/player/asset/recipe")
SPECS_PATH: Path = Path("features/entity/player/model/player_visual_specs.json")
DEFAULT_OUTPUT_ROOT: Path = Path("features/entity/player/assets/textures")

# <META - ROLE : Create a new 128x128 transparent RGBA image | L48-49>
def new_canvas() -> Image.Image:
    return Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))

# <META - ROLE : Render glowing wheel frame for 3-frame rotation animation | L52-82>
def render_wheel_frame(frame: int, total_frames: int, spec: dict[str, str | int]) -> Image.Image:
    """세로 캡슐형 발광 릴 코어 (3프레임 심리스 회전)."""
    img = new_canvas()
    cx, cy = 70.0, 40.0
    rx, ry = 5.0, 14.0

    glow_canvas = new_canvas()
    gd = ImageDraw.Draw(glow_canvas)
    gd.ellipse([cx - rx - 4, cy - ry - 4, cx + rx + 4, cy + ry + 4], fill=(56, 189, 248, 150))
    glow_canvas = glow_canvas.filter(ImageFilter.GaussianBlur(3))
    img.alpha_composite(glow_canvas)

    d = ImageDraw.Draw(img)

    d.rounded_rectangle([cx - rx - 1.5, cy - ry - 1.5, cx + rx + 1.5, cy + ry + 1.5], radius=5, fill="#040508")

    core_col = resolve_color("$wheel_core", spec)
    glow_col = resolve_color("$glow_color", spec)
    d.rounded_rectangle([cx - rx, cy - ry, cx + rx, cy + ry], radius=4, fill=core_col, outline=glow_col, width=2)

    rib_pitch = 5.5
    offset = (frame / float(total_frames)) * rib_pitch
    rib_col = resolve_color("$wheel_light", spec)

    for i in range(-3, 4):
        y_pos = cy + (i * rib_pitch) + offset
        if cy - ry + 2 <= y_pos <= cy + ry - 2:
            d.line([(cx - rx + 1.5, y_pos), (cx + rx - 1.5, y_pos)], fill=rib_col, width=2)

    d.line([(cx, cy - ry + 2), (cx, cy + ry - 2)], fill=(255, 255, 255, 220), width=1)
    return img

# <META - ROLE : Load player visual specs from SQLite with JSON fallback | L85-103>
def load_specs() -> dict[str, dict]:
    """SQLite SSOT에서 스펙을 조회하고, 실패 시 export된 JSON을 Fallback으로 사용합니다."""
    try:
        database = build_database(PROJECT_ROOT)
        rows = query_database(database, "SELECT * FROM entity_player_visual_specs ORDER BY rowid")
        if rows:
            specs: dict[str, dict] = {}
            for row in rows:
                pid = str(row.get("profile_id", ""))
                if pid:
                    specs[pid] = {k: v for k, v in row.items() if k != "profile_id"}
            return specs
    except Exception as error:
        print(f"[WARN] SQLite 플레이어 스펙 조회 실패, JSON Fallback 사용: {error}")

    if SPECS_PATH.exists():
        with open(SPECS_PATH, "r", encoding="utf-8") as f:
            return json.load(f)
    return {}

# <META - ROLE : Build 16-slot player atlas from profile data and shapes | L106-174>
def build_player_atlas(profile_id: str, profile_data: dict, out_dir: Path) -> Path:
    chassis_path = SHAPES_DIR / f"{profile_data.get('chassis_id', 'base_chassis_gem')}.json"
    visor_path = SHAPES_DIR / f"{profile_data.get('visor_id', 'base_visor_shield')}.json"
    face_path = SHAPES_DIR / f"{profile_data.get('face_pack_id', 'emoticons_cyber_neon')}.json"

    chassis_shape = load_shape_file(chassis_path)
    visor_shape = load_shape_file(visor_path)
    face_shape = load_shape_file(face_path)

    spec_dict: dict[str, str | int] = {
        "body_color": profile_data.get("body_color_hex", "#2C323D"),
        "body_dark": profile_data.get("body_dark_hex", "#161920"),
        "body_deep": profile_data.get("body_deep_hex", "#0E1015"),
        "body_bright": profile_data.get("body_bright_hex", "#475569"),
        "body_outline": profile_data.get("body_outline_hex", "#07080B"),
        "visor_color": profile_data.get("visor_color_hex", "#000000"),
        "eye_color": profile_data.get("eye_color_hex", "#38BDF8"),
        "glow_color": profile_data.get("glow_color_hex", "#7DD3FC"),
        "wheel_core": profile_data.get("wheel_core_hex", "#0284C7"),
        "wheel_light": profile_data.get("wheel_light_hex", "#FFFFFF"),
    }

    slots: list[Image.Image] = [new_canvas() for _ in range(TOTAL_SLOTS)]

    render_layer_commands(slots[0], chassis_shape.get("shadow", []), spec_dict)

    render_layer_commands(slots[1], chassis_shape.get("body", []), spec_dict)

    render_layer_commands(slots[2], chassis_shape.get("spine_stream", []), spec_dict)

    render_layer_commands(slots[3], visor_shape.get("visor_left", []), spec_dict)

    render_layer_commands(slots[4], visor_shape.get("visor_right", []), spec_dict)

    render_layer_commands(slots[5], visor_shape.get("visor_cracked_left", []), spec_dict)

    render_layer_commands(slots[6], visor_shape.get("visor_cracked_right", []), spec_dict)

    render_layer_commands(slots[7], face_shape.get("face_normal", []), spec_dict)

    render_layer_commands(slots[8], face_shape.get("face_hurt", []), spec_dict)

    render_layer_commands(slots[9], face_shape.get("face_dead", []), spec_dict)

    for i in range(3):
        slots[10 + i] = render_wheel_frame(i, 3, spec_dict)

    atlas = Image.new("RGBA", (ATLAS_WIDTH, ATLAS_HEIGHT), (0, 0, 0, 0))
    for idx, s_img in enumerate(slots):
        atlas.paste(s_img, (idx * CANVAS, 0), s_img)

    out_dir.mkdir(parents=True, exist_ok=True)
    sheet_path = out_dir / "sheet.png"
    atlas.save(sheet_path, "PNG")
    print(f"  [ATLAS] {sheet_path} ({ATLAS_WIDTH}x{ATLAS_HEIGHT}, 13 active + 3 TBD)")

    composite = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    composite.alpha_composite(slots[0])
    composite.alpha_composite(slots[1])
    composite.alpha_composite(slots[2])
    composite.alpha_composite(slots[10])
    composite.alpha_composite(slots[3])
    composite.alpha_composite(slots[4])
    composite.alpha_composite(slots[7])
    preview_path = out_dir / "preview_composite.png"
    composite.save(preview_path, "PNG")
    print(f"  [PREVIEW] {preview_path}")

    return sheet_path

# <META - ROLE : CLI entrypoint for batch player atlas generation | L177-205>
def main() -> None:
    parser = argparse.ArgumentParser(description="Generate player visual profile atlases")
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT_ROOT,
        help="output directory containing mouse_hero and profile subdirectories",
    )
    args = parser.parse_args()
    output_root = args.output

    specs = load_specs()
    if not specs:
        specs = {
            "HERO_CLASSIC_GEM": {
                "chassis_id": "base_chassis_gem",
                "visor_id": "base_visor_shield",
                "face_pack_id": "emoticons_cyber_neon",
            }
        }

    for pid, pdata in specs.items():
        print(f"\n→ 플레이어 프로필 [{pid}] 아틀라스 생성 중...")
        if pid == "HERO_CLASSIC_GEM":
            build_player_atlas(pid, pdata, output_root / "mouse_hero")
        out_profile_dir = output_root / pid.lower()
        build_player_atlas(pid, pdata, out_profile_dir)

    print("\n[SUCCESS] 플레이어 아틀라스 생성 완료.")

if __name__ == "__main__":
    main()
