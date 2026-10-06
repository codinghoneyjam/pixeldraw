# <META - FILE SUMMARY - Project module implementation>

import sys
from pathlib import Path

from PIL import Image, ImageDraw

TOOL_ROOT: Path = Path(__file__).resolve().parents[2]   # pixeldraw/
TARGET_ROOT: Path = TOOL_ROOT / "asset_work" / "target"  # recipes + legacy PNGs
sys.path.insert(0, str(TARGET_ROOT.parent))

# <META - ROLE : Execute pil line | L12-16>
def pil_line(pts, width):
    img = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.line(pts, fill=(255, 0, 0, 255), width=width)
    return set((x, y) for y in range(128) for x in range(128) if img.getpixel((x, y))[3] != 0)

# <META - ROLE : Execute pil poly | L19-23>
def pil_poly(pts, fill, outline, width):
    img = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.polygon(pts, fill=fill, outline=outline, width=width)
    return img

import subprocess, json, tempfile, os

JS = r"""
import { lineMask } from "./src/core/raster/segment.js";
import { polygonMask, polygonOutlineMask, polygonBBox } from "./src/core/raster/polygon.js";
import { rrectMask, outlineRing } from "./src/core/raster/raster_masks.js";

const out = {};

// bow line segment 1: (38,20)->(80,64) width 2
{
  const m = lineMask({x:38,y:20}, {x:80,y:64}, 2);
  const px = [];
  for (let y = 0; y < m.h; y++) for (let x = 0; x < m.w; x++) if (m.data[y*m.w+x]===1) px.push([m.x+x, m.y+y]);
  out.line_seg1_w2 = px;
}

// bow polygon 3: [[46,34],[58,52],[66,52]] fill-only
{
  const pts = [[46,34],[58,52],[66,52]];
  const bb = polygonBBox(pts);
  const local = pts.map(p => [p[0]-bb.x, p[1]-bb.y]);
  const m = polygonMask(local, bb.w, bb.h);
  const px = [];
  for (let y = 0; y < m.h; y++) for (let x = 0; x < m.w; x++) if (m.data[y*m.w+x]===1) px.push([bb.x+x, bb.y+y]);
  out.poly3_fill = px;
}

// bow polygon 1: fill+outline
{
  const pts = [[40,20],[52,28],[66,52],[58,52],[46,34],[36,22]];
  const bb = polygonBBox(pts);
  const local = pts.map(p => [p[0]-bb.x, p[1]-bb.y]);
  const fill = polygonMask(local, bb.w, bb.h);
  const ring = polygonOutlineMask(local, Math.max(bb.w, bb.h), 1);
  const fillPx = [], ringPx = [];
  for (let y = 0; y < fill.h; y++) for (let x = 0; x < fill.w; x++) if (fill.data[y*fill.w+x]===1) fillPx.push([bb.x+x, bb.y+y]);
  for (let y = 0; y < ring.h; y++) for (let x = 0; x < ring.w; x++) if (ring.data[y*ring.w+x]===1) ringPx.push([bb.x+x, bb.y+y]);
  // Same overlay caveat as the rounded rect: PIL's fill excludes the outline.
  const rLocal = new Set(ringPx.map(([x, y]) => [x - bb.x, y - bb.y].join(",")));
  out.poly1_fill = fillPx.filter(([x, y]) => !rLocal.has([x - bb.x, y - bb.y].join(",")));
  out.poly1_ring = ringPx;
}

// portal keycap_face rounded_rect [[26,16],[102,86]] r10 fill+outline w3
{
  const bx=26, by=16, bw=102-26+1, bh=86-16+1;
  const outer = rrectMask(bw, bh, 10);
  const ring = outlineRing("rrect", bw, bh, 10, 3);
  const outerPx = [], ringPx = [];
  for (let y = 0; y < outer.h; y++) for (let x = 0; x < outer.w; x++) if (outer.data[y*outer.w+x]===1) outerPx.push([bx+x, by+y]);
  for (let y = 0; y < ring.h; y++) for (let x = 0; x < ring.w; x++) if (ring.data[y*ring.w+x]===1) ringPx.push([bx+x, by+y]);
  // PIL draws the outline OVER the fill, so PIL's "fill" pixels are the mask
  // minus the ring. Comparing PIL's fill against the raw mask would report the
  // whole ring as a divergence and measure nothing at all.
  const ringLocal = new Set(ringPx.map(([x, y]) => [x - bx, y - by].join(",")));
  out.rrect_fill_only = outerPx.filter(([x, y]) => !ringLocal.has([x - bx, y - by].join(",")));
  out.rrect_ring = ringPx;
}

// portal skirt polygon 3: [[112,20],[112,108],[98,84],[98,16]] fill-only
{
  const pts = [[112,20],[112,108],[98,84],[98,16]];
  const bb = polygonBBox(pts);
  const local = pts.map(p => [p[0]-bb.x, p[1]-bb.y]);
  const m = polygonMask(local, bb.w, bb.h);
  const px = [];
  for (let y = 0; y < m.h; y++) for (let x = 0; x < m.w; x++) if (m.data[y*m.w+x]===1) px.push([bb.x+x, bb.y+y]);
  out.skirt3_fill = px;
}

console.log(JSON.stringify(out));
"""

with tempfile.NamedTemporaryFile("w", suffix=".mjs", delete=False, dir=str(TOOL_ROOT)) as f:
    f.write(JS)
    js_path = f.name

try:
    result = subprocess.run(["node", js_path], capture_output=True, text=True, cwd=str(TOOL_ROOT))
    if result.returncode != 0:
        print("JS ERROR:", result.stderr)
        sys.exit(1)
    js = json.loads(result.stdout)
finally:
    os.unlink(js_path)

# <META - ROLE : Execute jset | L107-108>
def jset(key):
    return set(tuple(p) for p in js[key])

pil = pil_line([(38, 20), (80, 64)], 2)
js_px = jset("line_seg1_w2")
print(f"line seg1 w2: PIL={len(pil)} JS={len(js_px)} onlyPIL={len(pil-js_px)} onlyJS={len(js_px-pil)}")
print("  onlyPIL:", sorted(pil - js_px))
print("  onlyJS:", sorted(js_px - pil))

pil_img = pil_poly([(46, 34), (58, 52), (66, 52)], (255, 0, 0, 255), None, 1)
pil_px = set((x, y) for y in range(128) for x in range(128) if pil_img.getpixel((x, y))[3] != 0)
js_px = jset("poly3_fill")
print(f"poly3 fill: PIL={len(pil_px)} JS={len(js_px)} onlyPIL={len(pil_px-js_px)} onlyJS={len(js_px-pil_px)}")
print("  onlyPIL:", sorted(pil_px - js_px))
print("  onlyJS:", sorted(js_px - pil_px))

pil_img = pil_poly([(40, 20), (52, 28), (66, 52), (58, 52), (46, 34), (36, 22)], (255, 0, 0, 255), (0, 255, 0, 255), 1)
pil_fill = set((x, y) for y in range(128) for x in range(128) if pil_img.getpixel((x, y))[:3] == (255, 0, 0))
pil_out = set((x, y) for y in range(128) for x in range(128) if pil_img.getpixel((x, y))[:3] == (0, 255, 0))
js_fill = jset("poly1_fill")
js_ring = jset("poly1_ring")
print(f"poly1 fill: PIL={len(pil_fill)} JS={len(js_fill)} onlyPIL={len(pil_fill-js_fill)} onlyJS={len(js_fill-pil_fill)}")
print("  fill onlyPIL:", sorted(pil_fill - js_fill))
print("  fill onlyJS:", sorted(js_fill - pil_fill))
print(f"poly1 ring: PIL={len(pil_out)} JS={len(js_ring)} onlyPIL={len(pil_out-js_ring)} onlyJS={len(js_ring-pil_out)}")
print("  ring onlyPIL:", sorted(pil_out - js_ring))
print("  ring onlyJS:", sorted(js_ring - pil_out))

pil_img = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
d = ImageDraw.Draw(pil_img)
d.rounded_rectangle([(26, 16), (102, 86)], radius=10, fill=(255, 0, 0, 255), outline=(0, 255, 0, 255), width=3)
pil_fill = set((x, y) for y in range(128) for x in range(128) if pil_img.getpixel((x, y))[:3] == (255, 0, 0))
pil_out = set((x, y) for y in range(128) for x in range(128) if pil_img.getpixel((x, y))[:3] == (0, 255, 0))
js_outer = jset("rrect_fill_only")
js_ring = jset("rrect_ring")
print(f"rrect fill: PIL={len(pil_fill)} JS={len(js_outer)} onlyPIL={len(pil_fill-js_outer)} onlyJS={len(js_outer-pil_fill)}")
print("  fill onlyPIL:", sorted(pil_fill - js_outer)[:40])
print("  fill onlyJS:", sorted(js_outer - pil_fill)[:40])
print(f"rrect ring: PIL={len(pil_out)} JS={len(js_ring)} onlyPIL={len(pil_out-js_ring)} onlyJS={len(js_ring-pil_out)}")
print("  ring onlyPIL:", sorted(pil_out - js_ring)[:40])
print("  ring onlyJS:", sorted(js_ring - pil_out)[:40])

pil_img = pil_poly([(112, 20), (112, 108), (98, 84), (98, 16)], (255, 0, 0, 255), None, 1)
pil_px = set((x, y) for y in range(128) for x in range(128) if pil_img.getpixel((x, y))[3] != 0)
js_px = jset("skirt3_fill")
print(f"skirt3 fill: PIL={len(pil_px)} JS={len(js_px)} onlyPIL={len(pil_px-js_px)} onlyJS={len(js_px-pil_px)}")
print("  onlyPIL:", sorted(pil_px - js_px))
print("  onlyJS:", sorted(js_px - pil_px))
