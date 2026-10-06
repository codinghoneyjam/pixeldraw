#!/usr/bin/env python3
# <META - FILE SUMMARY - Declarative vector path geometry engine for enemy sprite rendering>
"""
Declarative Vector Path Geometry Engine for Enemy Sprites
tools/assets/core/geometry.py

점 배열, 곡선/직선 세그먼트, 필렛 라운딩, 선 두께, 색상 토큰을 선언적으로 해석하여
1024x128 아틀라스 레이어를 100% 자동 생성하는 범용 벡터 렌더러.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any
from PIL import Image, ImageChops, ImageDraw, ImageFont

from .drawing import (
    _apply_mask,
    _draw_hud_brackets,
    _draw_scanlines,
    _draw_sheen_overlay,
    _rgba,
)

# <META - ROLE : Return vertices for a rectangle with stepped corner notches | L27-40>
def get_stepped_notch_polygon(
    x: int, y: int, w: int, h: int, notch_w: int, notch_h: int,
) -> list[tuple[int, int]]:
    """Return vertices for a rectangle with four stepped corner notches."""
    x0, y0 = x, y
    x1, y1 = x + w - 1, y + h - 1
    return [
        (x0, y0 + notch_h), (x0 + notch_w, y0 + notch_h),
        (x0 + notch_w, y0), (x1 - notch_w, y0),
        (x1 - notch_w, y0 + notch_h), (x1, y0 + notch_h),
        (x1, y1 - notch_h), (x1 - notch_w, y1 - notch_h),
        (x1 - notch_w, y1), (x0 + notch_w, y1),
        (x0 + notch_w, y1 - notch_h), (x0, y1 - notch_h),
    ]

# <META - ROLE : Return a rectangle with two diagonally-opposite chamfered corners | L43-63>
def get_asymmetric_chamfer_polygon(
    x: int, y: int, w: int, h: int, chamfer: int, corners: str = "TL_BR",
) -> list[tuple[int, int]]:
    """Return a rectangle polygon with two diagonally-opposite corners chamfered.

    corners: "TL_BR" chamfers top-left + bottom-right, "TR_BL" chamfers
    top-right + bottom-left. Other corners stay square.
    """
    c = max(0, min(chamfer, w // 2, h // 2))
    x0, y0 = x, y
    x1, y1 = x + w - 1, y + h - 1
    if corners == "TR_BL":
        return [
            (x0, y0), (x1 - c, y0), (x1, y0 + c),
            (x1, y1), (x0 + c, y1), (x0, y1 - c),
        ]

    return [
        (x0 + c, y0), (x1, y0), (x1, y1 - c),
        (x1 - c, y1), (x0, y1), (x0, y0 + c),
    ]

# <META - ROLE : Return four corner L-bracket polylines for HUD framing | L66-78>
def get_bracket_segments(
    x0: int, y0: int, x1: int, y1: int, arm: int,
) -> list[list[tuple[int, int]]]:
    """Return 4 corner L-bracket polylines (┌ ┐ └ ┘) for HUD-style framing.

    Each segment is a 3-point polyline suitable for ImageDraw.line.
    """
    return [
        [(x0 + arm, y0), (x0, y0), (x0, y0 + arm)],
        [(x1 - arm, y0), (x1, y0), (x1, y0 + arm)],
        [(x0 + arm, y1), (x0, y1), (x0, y1 - arm)],
        [(x1 - arm, y1), (x1, y1), (x1, y1 - arm)],
    ]

# <META - ROLE : Adjust hex color brightness by a factor | L81-90>
def _shade_hex(hex_str: str, factor: float) -> str:
    """헥사 색상의 명도를 factor 배율로 조절."""
    h = hex_str.lstrip("#")
    if len(h) < 6:
        return hex_str
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    r = min(255, max(0, int(r * factor)))
    g = min(255, max(0, int(g * factor)))
    b = min(255, max(0, int(b * factor)))
    return f"#{r:02X}{g:02X}{b:02X}"

# <META - ROLE : Resolve color tokens and hex strings to RGBA tuples | L93-120>
def resolve_color(
    val: Any,
    spec: dict[str, Any],
    default: tuple[int, int, int, int] = (255, 255, 255, 255),
) -> tuple[int, int, int, int]:
    """$body_color 및 몸통톤 기반 파생 토큰을 RGBA 튜플로 변환."""
    if isinstance(val, (list, tuple)):
        if len(val) == 3:
            return (val[0], val[1], val[2], 255)
        return (val[0], val[1], val[2], val[3])
    if not isinstance(val, str):
        return default

    if val.startswith("$"):
        token = val[1:]
        body_hex = spec.get("body_color", spec.get("body_color_hex", "#808080"))
        if token == "body_outline":
            val = "#000000"
        elif token == "body_dark":
            val = _shade_hex(body_hex, 0.60)
        elif token == "body_deep":
            val = _shade_hex(body_hex, 0.22)
        elif token in ("body_light", "body_bright"):
            val = _shade_hex(body_hex, 1.25)
        else:
            val = spec.get(token, spec.get(f"{token}_hex", "#FFFFFF"))

    return _rgba(val)

# <META - ROLE : Generate rounded corner arcs for a polygon | L123-173>
def fillet_polygon(pts: list[tuple[int, int]], radius: float = 6.0) -> list[tuple[int, int]]:
    """다각형 꼭짓점에 내접 원호(Arc)를 기하학적으로 생성하여 둥근 모서리 점 배열 반환."""
    if radius <= 0.0 or len(pts) < 3:
        return pts
    import math
    n = len(pts)
    new_pts: list[tuple[int, int]] = []
    for i in range(n):
        p_prev = pts[(i - 1) % n]
        p_curr = pts[i]
        p_next = pts[(i + 1) % n]
        v1 = (p_prev[0] - p_curr[0], p_prev[1] - p_curr[1])
        v2 = (p_next[0] - p_curr[0], p_next[1] - p_curr[1])
        l1 = math.hypot(v1[0], v1[1])
        l2 = math.hypot(v2[0], v2[1])
        if l1 == 0 or l2 == 0:
            new_pts.append(p_curr)
            continue
        u1 = (v1[0] / l1, v1[1] / l1)
        u2 = (v2[0] / l2, v2[1] / l2)
        dot = max(-0.999, min(0.999, u1[0] * u2[0] + u1[1] * u2[1]))
        half_angle = math.acos(dot) / 2.0
        tan_half = math.tan(half_angle)
        if tan_half < 0.001:
            new_pts.append(p_curr)
            continue

        max_r = min(radius, (l1 * 0.45) * tan_half, (l2 * 0.45) * tan_half)
        tangent_len = max_r / tan_half
        p_start = (p_curr[0] + u1[0] * tangent_len, p_curr[1] + u1[1] * tangent_len)
        p_end = (p_curr[0] + u2[0] * tangent_len, p_curr[1] + u2[1] * tangent_len)
        bisector = (u1[0] + u2[0], u1[1] + u2[1])
        b_len = math.hypot(bisector[0], bisector[1])
        if b_len < 0.001:
            new_pts.append(p_curr)
            continue
        w = (bisector[0] / b_len, bisector[1] / b_len)
        center_dist = max_r / math.sin(half_angle)
        center = (p_curr[0] + w[0] * center_dist, p_curr[1] + w[1] * center_dist)

        a_start = math.atan2(p_start[1] - center[1], p_start[0] - center[0])
        a_end = math.atan2(p_end[1] - center[1], p_end[0] - center[0])

        diff = (a_end - a_start + math.pi) % (2.0 * math.pi) - math.pi
        steps = 8
        for s in range(steps + 1):
            ang = a_start + diff * (s / float(steps))
            arc_x = center[0] + max_r * math.cos(ang)
            arc_y = center[1] + max_r * math.sin(ang)
            new_pts.append((int(round(arc_x)), int(round(arc_y))))
    return new_pts

# <META - ROLE : Delegate a single vector command to the v2 engine | L176-184>
def render_command(
    d: ImageDraw.ImageDraw,
    cmd: dict[str, Any],
    spec: dict[str, Any],
    base_img: Image.Image | None = None,
) -> None:
    """단일 선언적 벡터 커맨드 해석 및 실행 (v2 엔진 위임 브릿지)."""
    from dev.tools.assets.v2 import engine as _engine
    _engine.render_command(d, cmd, spec, base_img=base_img)

# <META - ROLE : Render a list of layer commands sequentially | L187-191>
def render_layer_commands(img: Image.Image, commands: list[dict[str, Any]], spec: dict[str, Any]) -> None:
    """레이어 커맨드 리스트를 순차 렌더링."""
    d = ImageDraw.Draw(img)
    for cmd in commands:
        render_command(d, cmd, spec, base_img=img)

# <META - ROLE : Render a visor layer with mask, scanlines, and HUD brackets | L194-218>
def render_visor_layer(visor_def: dict[str, Any], spec: dict[str, Any], size: tuple[int, int] = (128, 128)) -> Image.Image:
    """바이저 영역을 마스킹하여 검정 바탕, 네온 림, CRT 스캔라인, HUD 꺾쇠를 렌더링."""
    base = Image.new("RGBA", size, (0, 0, 0, 0))
    shape_cmd = visor_def.get("shape", {})
    if not shape_cmd:
        return base

    render_command(ImageDraw.Draw(base), shape_cmd, spec, base_img=base)

    mask = Image.new("L", size, 0)
    render_command(ImageDraw.Draw(mask), shape_cmd, spec)

    overlay = Image.new("RGBA", size, (0, 0, 0, 0))
    d_ov = ImageDraw.Draw(overlay)

    crt = visor_def.get("crt_lines")
    if crt:
        y0, y1, step = crt.get("y_range", [45, 80, 3])
        x0, x1 = crt.get("x_range", [20, 108])
        _draw_scanlines(d_ov, x0, y0, x1, y1, step=step, alpha=45)

    masked_a = ImageChops.multiply(overlay.getchannel("A"), mask)
    overlay.putalpha(masked_a)
    base.alpha_composite(overlay)
    return base

# <META - ROLE : Load a geometry spec JSON file with base template inheritance | L221-224>
def load_shape_file(file_path: str | Path) -> dict[str, Any] | None:
    """JSON 경로에서 지오메트리 명세 로드 및 base 템플릿 상속/합성 지원 (v2 위임 브릿지)."""
    from dev.tools.assets.v2 import engine as _engine
    return _engine.load_shape_file(file_path)

# <META - ROLE : Render a named layer from a shape spec to RGBA image | L227-235>
def render_shape_layer(
    shape_spec: dict[str, Any],
    layer_name: str,
    spec: dict[str, Any],
    size: tuple[int, int] = (128, 128),
) -> Image.Image:
    """shape_spec의 지정 레이어를 128x128 RGBA 이미지로 렌더링 (v2 엔진 위임 브릿지)."""
    from dev.tools.assets.v2 import engine as _engine
    return _engine.render_shape_layer(shape_spec, layer_name, spec, size)
