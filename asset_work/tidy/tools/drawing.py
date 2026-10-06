#!/usr/bin/env python3
# <META - FILE SUMMARY - Drawing primitive utilities shared by enemy asset generators>
"""
Drawing primitive utilities for enemy asset generation.
tools/assets/core/drawing.py

공용 드로잉 헬퍼 모음. 적 생성기와 선언적 지오메트리 렌더러에서 공유.
"""

from __future__ import annotations

import math
from typing import Callable

try:
    from PIL import Image, ImageChops, ImageDraw
except ImportError:
    raise ImportError("[ERROR] Pillow 미설치. 실행: pip install Pillow")

CANVAS: int = 128

# <META - ROLE : Convert hex color string to RGBA tuple | L23-26>
def _rgba(hex_color: str, a: int = 255) -> tuple[int, int, int, int]:
    """HEX 색상 문자열(#RRGGBB)을 RGBA 튜플로 변환."""
    h = hex_color.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), a)

# <META - ROLE : Create a 128x128 transparent RGBA canvas and draw context | L29-32>
def _new_canvas() -> tuple[Image.Image, ImageDraw.ImageDraw]:
    """128×128 투명 RGBA 캔버스와 드로우 컨텍스트를 생성하여 반환."""
    img = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    return img, ImageDraw.Draw(img)

# <META - ROLE : Compute ellipse bounding box from center and half-axes | L35-37>
def _ebb(cx: float, cy: float, rx: float, ry: float) -> list[tuple[float, float]]:
    """ellipse bounding box: center + half-axes → [(x0,y0),(x1,y1)]"""
    return [(cx - rx, cy - ry), (cx + rx, cy + ry)]

# <META - ROLE : Draw content clipped by a mask onto target canvas | L40-58>
def _apply_mask(
    target_draw: ImageDraw.ImageDraw,
    draw_content_fn: Callable[[ImageDraw.ImageDraw], None],
    draw_mask_fn: Callable[[ImageDraw.ImageDraw], None],
) -> None:
    """마스크 영역 내부에만 컨텐츠를 그리고 타겟 캔버스에 알파 블렌딩."""
    overlay = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    od = ImageDraw.Draw(overlay)
    draw_content_fn(od)

    mask = Image.new("L", (CANVAS, CANVAS), 0)
    md = ImageDraw.Draw(mask)
    draw_mask_fn(md)

    overlay_a = overlay.getchannel("A")
    masked_a = ImageChops.multiply(overlay_a, mask)
    overlay.putalpha(masked_a)

    target_draw._image.alpha_composite(overlay)

# <META - ROLE : Draw four corner L-bracket HUD frame lines | L61-80>
def _draw_hud_brackets(
    d: ImageDraw.ImageDraw,
    x0: int, y0: int, x1: int, y1: int,
    arm: int = 4,
    color: tuple[int, int, int, int] = (255, 255, 255, 220),
    width: int = 1,
) -> None:
    """바이저 모서리 4군데에 사각형 하양 꺾쇠 [ ] HUD 브래킷 렌더링."""

    d.line([(x0, y0), (x0 + arm, y0)], fill=color, width=width)
    d.line([(x0, y0), (x0, y0 + arm)], fill=color, width=width)

    d.line([(x1, y0), (x1 - arm, y0)], fill=color, width=width)
    d.line([(x1, y0), (x1, y0 + arm)], fill=color, width=width)

    d.line([(x0, y1), (x0 + arm, y1)], fill=color, width=width)
    d.line([(x0, y1), (x0, y1 - arm)], fill=color, width=width)

    d.line([(x1, y1), (x1 - arm, y1)], fill=color, width=width)
    d.line([(x1, y1), (x1, y1 - arm)], fill=color, width=width)

# <META - ROLE : Draw subtle CRT horizontal scanlines in a region | L83-92>
def _draw_scanlines(
    d: ImageDraw.ImageDraw,
    x0: int, y0: int, x1: int, y1: int,
    step: int = 3,
    alpha: int = 45,
) -> None:
    """바이저 내부 은은한 CRT 디지털 가로 스캔라인 렌더링."""
    col = (0, 0, 0, alpha)
    for y in range(y0 + 2, y1 - 1, step):
        d.line([(x0 + 2, y), (x1 - 2, y)], fill=col, width=1)

# <META - ROLE : Draw diagonal glass reflection sheen lines on visor | L95-108>
def _draw_sheen_overlay(
    d: ImageDraw.ImageDraw,
    mask_fn: Callable[[ImageDraw.ImageDraw], None],
    p1: tuple[int, int] = (38, 42),
    p2: tuple[int, int] = (26, 76),
    dx: int = 5,
) -> None:
    """표정 위에 얹어지는 바이저 마스킹 대각선 유리 반사광 2선 (주선 + 보조선)."""
    # <META - ROLE : Draw two diagonal sheen lines on the overlay | L0-0>
    def _draw_lines(od: ImageDraw.ImageDraw) -> None:
        od.line([p1, p2], fill=(255, 255, 255, 110), width=2)
        od.line([(p1[0] + dx, p1[1]), (p2[0] + dx, p2[1])], fill=(255, 255, 255, 60), width=1)

    _apply_mask(d, _draw_lines, mask_fn)

# <META - ROLE : Generate rounded triangle polygon points with fillets | L111-141>
def _get_rounded_triangle_pts(
    p_top: tuple[float, float],
    p_right: tuple[float, float],
    p_left: tuple[float, float],
    fillet_r: float = 6.0,
) -> list[tuple[int, int]]:
    """정삼각형의 3개 꼭지점을 완만한 호(fillet)로 둥글린 다각형 점 리스트 생성."""
    corners = [p_top, p_right, p_left]
    pts: list[tuple[int, int]] = []
    n = len(corners)
    for i in range(n):
        curr = corners[i]
        prev_pt = corners[(i - 1) % n]
        next_pt = corners[(i + 1) % n]

        v_prev = (prev_pt[0] - curr[0], prev_pt[1] - curr[1])
        len_prev = math.hypot(v_prev[0], v_prev[1])
        u_prev = (v_prev[0] / len_prev, v_prev[1] / len_prev)

        v_next = (next_pt[0] - curr[0], next_pt[1] - curr[1])
        len_next = math.hypot(v_next[0], v_next[1])
        u_next = (v_next[0] / len_next, v_next[1] / len_next)

        p_start = (curr[0] + u_prev[0] * fillet_r, curr[1] + u_prev[1] * fillet_r)
        p_end = (curr[0] + u_next[0] * fillet_r, curr[1] + u_next[1] * fillet_r)

        pts.append((int(round(p_start[0])), int(round(p_start[1]))))
        pts.append((int(round(curr[0] * 0.4 + (p_start[0] + p_end[0]) * 0.3)),
                    int(round(curr[1] * 0.4 + (p_start[1] + p_end[1]) * 0.3))))
        pts.append((int(round(p_end[0])), int(round(p_end[1]))))
    return pts

