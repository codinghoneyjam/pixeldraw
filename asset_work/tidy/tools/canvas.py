#!/usr/bin/env python3
# <META - FILE SUMMARY - Generic canvas drawing tool that renders JSON recipes to RGBA PNG>
"""
Generic Canvas Drawing Tool
tools/assets/core/canvas.py

JSON 레시피 파일을 읽어 도형을 그리고 128*128(또는 지정 크기) RGBA PNG로 출력합니다.
여러 레이어를 순서대로 합성하며, 색상·선 두께·반지름 등을 자유롭게 지정합니다.

Usage:
  python tools/assets/generators/draw_canvas.py recipe.json
  python tools/assets/generators/draw_canvas.py recipe.json -o custom.png
  python tools/assets/generators/draw_canvas.py recipe.json --canvas 256x256
  python tools/assets/generators/draw_canvas.py --template circle   # 예제 JSON 출력
  python tools/assets/generators/draw_canvas.py --template all      # 전체 데모
  python tools/assets/generators/draw_canvas.py --list-ops          # 지원 연산 목록
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Any

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

try:
    from PIL import Image, ImageChops, ImageDraw, ImageFont
except ImportError:
    print("[ERROR] Pillow 미설치: pip install Pillow")
    sys.exit(1)

# <META - ROLE : Parse hex string or list color notation into RGBA tuple | L35-48>
def _parse_color(c: Any) -> tuple[int, int, int, int] | None:
    """#RRGGBB / #RRGGBBAA / [R,G,B] / [R,G,B,A] → RGBA tuple. None 허용."""
    if c is None:
        return None
    if isinstance(c, (list, tuple)):
        r, g, b = int(c[0]), int(c[1]), int(c[2])
        return (r, g, b, int(c[3]) if len(c) > 3 else 255)
    if isinstance(c, str):
        h = c.lstrip("#")
        if len(h) == 6:
            return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), 255)
        if len(h) == 8:
            return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), int(h[6:8], 16))
    raise ValueError(f"지원하지 않는 색상 형식: {c!r}  (#RRGGBB / #RRGGBBAA / [R,G,B,A])")

# <META - ROLE : Extract bounding box from an operation dict | L51-60>
def _bbox(op: dict) -> list[tuple]:
    """op에서 bounding box [(x0,y0),(x1,y1)] 추출.
    bbox 키 우선; 없으면 center + radius(or rx/ry) 조합."""
    if "bbox" in op:
        b = op["bbox"]
        return [(b[0][0], b[0][1]), (b[1][0], b[1][1])]
    cx, cy = op["center"]
    rx = op.get("radius", op.get("rx", 1))
    ry = op.get("ry", rx)
    return [(cx - rx, cy - ry), (cx + rx, cy + ry)]

# <META - ROLE : Extract point list from an operation dict | L63-64>
def _pts(op: dict) -> list[tuple]:
    return [tuple(p) for p in op["points"]]

# <META - ROLE : Draw filled shapes (circle, ellipse, rect, rounded_rect) | L67-77>
def _draw_filled(d: ImageDraw.ImageDraw, op: dict, fill, outline, width: int) -> None:
    """circle / ellipse / rect / rounded_rect"""
    t = op["type"]
    bb = _bbox(op)
    if t in ("circle", "ellipse"):
        d.ellipse(bb, fill=fill, outline=outline, width=width)
    elif t == "rect":
        d.rectangle(bb, fill=fill, outline=outline, width=width)
    elif t == "rounded_rect":
        r = int(op.get("radius", 8))
        d.rounded_rectangle(bb, radius=r, fill=fill, outline=outline, width=width)

# <META - ROLE : Draw polygon, line, or polyline paths | L80-86>
def _draw_path(d: ImageDraw.ImageDraw, op: dict, fill, outline, width: int) -> None:
    """polygon / line / polyline"""
    t = op["type"]
    if t == "polygon":
        d.polygon(_pts(op), fill=fill, outline=outline, width=width)
    elif t in ("line", "polyline"):
        d.line(_pts(op), fill=fill or outline, width=width)

# <META - ROLE : Draw arc, chord, or pieslice shapes | L89-100>
def _draw_arc_type(d: ImageDraw.ImageDraw, op: dict, fill, outline, width: int) -> None:
    """arc / chord / pieslice"""
    t = op["type"]
    bb = _bbox(op)
    start = float(op.get("start", 0))
    end = float(op.get("end", 360))
    if t == "arc":
        d.arc(bb, start=start, end=end, fill=fill or outline, width=width)
    elif t == "chord":
        d.chord(bb, start=start, end=end, fill=fill, outline=outline, width=width)
    elif t == "pieslice":
        d.pieslice(bb, start=start, end=end, fill=fill, outline=outline, width=width)

# <META - ROLE : Draw point or text elements | L103-127>
def _draw_misc(d: ImageDraw.ImageDraw, op: dict, fill, outline, width: int) -> None:
    """point / text"""
    t = op["type"]
    if t == "point":
        pts = [tuple(op["pos"])] if "pos" in op else _pts(op)
        d.point(pts, fill=fill)
    elif t == "text":
        pos = tuple(op.get("pos", [0, 0]))
        size = int(op.get("font_size", 16))
        try:
            font = ImageFont.truetype(op.get("font", "arial.ttf"), size)
        except Exception:
            try:
                font = ImageFont.load_default(size=size)
            except Exception:
                font = None
        d.text(
            pos,
            str(op.get("text", "")),
            fill=fill,
            font=font,
            anchor=op.get("anchor", "lt"),
            stroke_width=int(op.get("stroke_width", 0)),
            stroke_fill=_parse_color(op.get("stroke_fill", op.get("fill", op.get("color")))),
        )

_DISPATCH: dict = {
    "circle": _draw_filled, "ellipse": _draw_filled,
    "rect": _draw_filled, "rounded_rect": _draw_filled,
    "polygon": _draw_path, "line": _draw_path, "polyline": _draw_path,
    "arc": _draw_arc_type, "chord": _draw_arc_type, "pieslice": _draw_arc_type,
    "point": _draw_misc, "text": _draw_misc,
}

# <META - ROLE : Dispatch a drawing operation to the appropriate shape handler | L138-147>
def _draw_op(d: ImageDraw.ImageDraw, op: dict) -> None:
    t = op.get("type", "")
    fn = _DISPATCH.get(t)
    if fn is None:
        print(f"  [WARN] 알 수 없는 type: {t!r} (--list-ops 참조)")
        return
    fill = _parse_color(op.get("fill", op.get("color")))
    outline = _parse_color(op.get("outline"))
    width = int(op.get("width", op.get("outline_width", 1)))
    fn(d, op, fill, outline, width)

# <META - ROLE : Erase shape alpha from image to create transparent gaps | L150-181>
def _erase_op(img: Image.Image, op: dict) -> None:
    """레이어의 도형 영역 알파를 0으로 파내어 투명 틈(간격)을 만든다.

    `"op": "erase"` 인 레이어에만 적용된다. 도형 실루엣을 8-bit L 마스크(255)로
    그린 뒤 원본 알파에서 그 영역을 제거한다. 색-투명링-색 3층 구조 구현에 사용한다.
    L 마스크는 단일 채널이므로 색상값 대신 정수 255로 채운다.
    """
    mask = Image.new("L", img.size, 0)
    md = ImageDraw.Draw(mask)
    t = op.get("type", "")
    if t == "polygon":
        md.polygon(_pts(op), fill=255)
    elif t in ("circle", "ellipse"):
        md.ellipse(_bbox(op), fill=255)
    elif t == "rect":
        md.rectangle(_bbox(op), fill=255)
    elif t == "rounded_rect":
        md.rounded_rectangle(_bbox(op), radius=int(op.get("radius", 8)), fill=255)
    elif t in ("line", "polyline"):
        md.line(_pts(op), fill=255, width=int(op.get("width", 1)))
    elif t == "arc":
        md.arc(
            _bbox(op),
            start=float(op.get("start", 0)),
            end=float(op.get("end", 360)),
            fill=255,
            width=int(op.get("width", 1)),
        )
    else:
        return
    keep = Image.eval(mask, lambda v: 255 - v)
    img.putalpha(ImageChops.multiply(img.getchannel("A"), keep))

# <META - ROLE : Delegate JSON recipe execution to v2 engine and return output path | L184-188>
def run_recipe(recipe: dict, output_override: str | None = None,
               size_override: str | None = None) -> Path:
    """JSON 레시피를 실행하여 PNG로 저장하고 경로를 반환 (v2 엔진 위임 브릿지)."""
    from dev.tools.assets.v2 import engine as _engine
    return _engine.run_recipe_compat(recipe, output_override=output_override, size_override=size_override)

_CANVAS_128 = {"width": 128, "height": 128, "background": [0, 0, 0, 0]}

TEMPLATES: dict[str, dict] = {
    "circle": {
        "canvas": _CANVAS_128, "output": "output_circle.png",
        "layers": [{"type": "circle", "center": [64, 64], "radius": 40,
                    "fill": "#4488FF", "outline": "#2255CC", "width": 2}]
    },
    "ellipse": {
        "canvas": _CANVAS_128, "output": "output_ellipse.png",
        "layers": [{"type": "ellipse", "bbox": [[16, 32], [112, 96]],
                    "fill": "#FF8800AA", "outline": "#CC5500", "width": 2}]
    },
    "rounded_rect": {
        "canvas": _CANVAS_128, "output": "output_rounded_rect.png",
        "layers": [{"type": "rounded_rect", "bbox": [[16, 16], [112, 112]],
                    "radius": 16, "fill": "#22CC66", "outline": "#116633", "width": 2}]
    },
    "polygon": {
        "canvas": _CANVAS_128, "output": "output_polygon.png",
        "layers": [{"type": "polygon",
                    "points": [[64, 8], [8, 120], [120, 120]],
                    "fill": "#DD2244", "outline": "#880011", "width": 2}]
    },
    "arc": {
        "canvas": _CANVAS_128, "output": "output_arc.png",
        "layers": [{"type": "arc", "center": [64, 64], "radius": 48,
                    "start": 0, "end": 270, "fill": "#FFCC00", "width": 5}]
    },
    "chord": {
        "canvas": _CANVAS_128, "output": "output_chord.png",
        "layers": [{"type": "chord", "center": [64, 64], "radius": 48,
                    "start": 180, "end": 360,
                    "fill": "#8844FF", "outline": "#4422AA", "width": 2}]
    },
    "line": {
        "canvas": {"width": 128, "height": 128, "background": [20, 20, 30, 255]},
        "output": "output_line.png",
        "layers": [
            {"type": "line", "points": [[8, 8], [120, 120]], "fill": "#00FFAA", "width": 3},
            {"type": "line", "points": [[8, 120], [120, 8]], "fill": "#FF4466", "width": 3},
            {"type": "polyline",
             "points": [[64, 8], [8, 64], [64, 120], [120, 64], [64, 8]],
             "fill": "#FFFF00", "width": 2},
        ]
    },
    "text": {
        "canvas": {"width": 256, "height": 64, "background": [30, 30, 30, 255]},
        "output": "output_text.png",
        "layers": [{"type": "text", "pos": [128, 32], "text": "Hello Canvas!",
                    "fill": "#FFFFFF", "font_size": 24, "anchor": "mm"}]
    },
    "all": {
        "canvas": {"width": 256, "height": 256, "background": [20, 20, 30, 255]},
        "output": "output_all_demo.png",
        "layers": [
            {"type": "rounded_rect", "bbox": [[10, 10], [246, 246]],
             "radius": 20, "fill": "#1A2050", "outline": "#4466AA", "width": 2},
            {"type": "circle", "center": [64, 64], "radius": 36,
             "fill": "#FF446688", "outline": "#FF0033", "width": 2},
            {"type": "ellipse", "bbox": [[140, 20], [240, 80]], "fill": "#44FFAA88"},
            {"type": "polygon", "points": [[128, 90], [90, 160], [166, 160]],
             "fill": "#FFCC00", "outline": "#AA8800", "width": 2},
            {"type": "arc", "center": [64, 185], "radius": 44,
             "start": 180, "end": 360, "fill": "#00CCFF", "width": 4},
            {"type": "chord", "center": [192, 185], "radius": 34,
             "start": 0, "end": 180, "fill": "#FF8800"},
            {"type": "pieslice", "center": [192, 70], "radius": 30,
             "start": 225, "end": 315, "fill": "#AAFFAA", "outline": "#55CC55", "width": 1},
            {"type": "line", "points": [[10, 128], [246, 128]],
             "fill": "#FFFFFF44", "width": 1},
            {"type": "polyline", "points": [[10, 20], [246, 20], [246, 236], [10, 236], [10, 20]],
             "fill": "#FFFFFF33", "width": 1},
            {"type": "text", "pos": [128, 243], "text": "draw_canvas demo",
             "fill": "#99AABB", "font_size": 13, "anchor": "mm"},
        ]
    },
}

LIST_OPS_TEXT = """
지원 도형 type 목록:
  circle        원           center + radius  (또는 bbox)
  ellipse       타원         bbox  (또는 center + rx + ry)
  rect          직사각형     bbox
  rounded_rect  둥근 사각형  bbox + radius
  polygon       다각형       points: [[x,y], ...]
  line          직선         points: [[x0,y0], [x1,y1]]
  polyline      꺾은선       points: 3개 이상
  arc           호(외곽선)   bbox/center + start/end + width
  chord         현(채움)     bbox/center + start/end
  pieslice      부채꼴       bbox/center + start/end
  point         점           pos: [x,y]
  text          텍스트       pos + text + font_size + anchor

공통 파라미터:
  fill          채우기 색상   "#RRGGBB" | "#RRGGBBAA" | [R,G,B] | [R,G,B,A]
  outline       외곽선 색상   (동일 형식)
  width         선/외곽선 두께 (정수, 기본 1)
  radius        모서리 반지름 (rounded_rect 전용, 기본 8)

arc/chord 각도 기준: 0=3시 방향, 90=6시, 180=9시, 270=12시 (시계방향)

bbox 형식:  {"bbox": [[x0,y0], [x1,y1]]}
원 형식:    {"center": [cx,cy], "radius": r}
타원 형식:  {"center": [cx,cy], "rx": rx, "ry": ry}
"""

Color = tuple[int, int, int, int]

# <META - ROLE : Draw a beveled border around a polygon | L300-323>
def draw_bevel_frame(
    draw: "ImageDraw.ImageDraw",
    points: list[tuple[int, int]],
    highlight: Color,
    shade: Color,
    width: int = 2,
) -> None:
    """Draw a bevel around a polygon: top/left edges get highlight, bottom/right get shade.

    Edges are classified by their midpoint direction relative to the polygon centroid.
    """
    n = len(points)
    if n < 3:
        return
    cx = sum(p[0] for p in points) / n
    cy = sum(p[1] for p in points) / n
    for i in range(n):
        a = points[i]
        b = points[(i + 1) % n]
        mx = (a[0] + b[0]) / 2.0
        my = (a[1] + b[1]) / 2.0

        is_light = (my < cy) or (mx < cx and abs(my - cy) < 1.0)
        draw.line([a, b], fill=highlight if is_light else shade, width=width)

# <META - ROLE : Fill a rounded-rect visor with glass color and rim | L326-338>
def fill_visor_glass(
    draw: "ImageDraw.ImageDraw",
    bbox: list[tuple[int, int]],
    radius: int,
    glass: Color,
    rim: Color,
    width: int = 3,
) -> None:
    """Fill a rounded-rect visor with dark glass + neon rim + top specular streak."""
    draw.rounded_rectangle(bbox, radius=radius, fill=glass, outline=rim, width=width)
    (x0, y0), (x1, _y1) = bbox
    streak = (rim[0], rim[1], rim[2], 120)
    draw.line([(x0 + 6, y0 + 5), (x1 - 6, y0 + 5)], fill=streak, width=2)

# <META - ROLE : Overlay horizontal CRT scanlines within a bounding box | L341-350>
def composite_scanlines(
    draw: "ImageDraw.ImageDraw",
    bbox: list[tuple[int, int]],
    step: int,
    color: Color,
) -> None:
    """Overlay horizontal CRT scanlines within bbox."""
    (x0, y0), (x1, y1) = bbox
    for y in range(y0 + step, y1, step):
        draw.line([(x0, y), (x1, y)], fill=color, width=1)

# <META - ROLE : CLI entry point for the canvas drawing tool | L353-389>
def main() -> None:
    parser = argparse.ArgumentParser(
        description="범용 캔버스 드로잉 툴 — JSON 레시피로 도형을 그려 PNG 출력",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="예: python draw_canvas.py recipe.json -o out.png",
    )
    parser.add_argument("recipe", nargs="?", help="JSON 레시피 파일 경로")
    parser.add_argument("-o", "--output", help="출력 PNG 경로 (레시피 설정 덮어쓰기)")
    parser.add_argument("--canvas", metavar="WxH", help="캔버스 크기 덮어쓰기 (예: 256x256)")
    parser.add_argument("--template", nargs="?", const="all", metavar="NAME",
                        help=f"예제 JSON 출력. 선택: {', '.join(TEMPLATES)}")
    parser.add_argument("--list-ops", action="store_true", help="지원 연산 목록 출력")
    args = parser.parse_args()

    if args.list_ops:
        print(LIST_OPS_TEXT)
        return

    if args.template is not None:
        name = args.template if args.template in TEMPLATES else "all"
        if args.template not in TEMPLATES:
            print(f"[WARN] '{args.template}' 없음. 사용 가능: {', '.join(TEMPLATES)}\n")
        print(json.dumps(TEMPLATES[name], indent=2, ensure_ascii=False))
        return

    if not args.recipe:
        parser.print_help()
        return

    path = Path(args.recipe)
    if not path.exists():
        print(f"[ERROR] 파일 없음: {path}")
        sys.exit(1)

    recipe = json.loads(path.read_text(encoding="utf-8"))
    out = run_recipe(recipe, output_override=args.output, size_override=args.canvas)
    print(f"[OK] → {out}")

if __name__ == "__main__":
    main()
