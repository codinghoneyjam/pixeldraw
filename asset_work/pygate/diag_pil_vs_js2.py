# <META - FILE SUMMARY - Project module implementation>

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw

TOOL_ROOT: Path = Path(__file__).resolve().parents[2]   # pixeldraw/
TARGET_ROOT: Path = TOOL_ROOT / "asset_work" / "target"  # recipes + legacy PNGs
DT = TOOL_ROOT

JS = r"""
import { polygonMask, polygonOutlineMask, polygonBBox } from "./src/core/raster/polygon.js";

const out = {};

// wide line via polygonOutlineMask (width>1 uses wideLineQuadEdges + polygonGeneric fill)
function wideLine(x0, y0, x1, y1, width) {
  const m = polygonOutlineMask([[x0, y0], [x1, y1]], 128, width);
  const px = [];
  for (let y = 0; y < m.h; y++) for (let x = 0; x < m.w; x++) if (m.data[y * m.w + x] === 1) px.push([x, y]);
  return px;
}
out.wide_line_seg1 = wideLine(38, 20, 80, 64, 2);
out.wide_line_seg2 = wideLine(80, 64, 38, 108, 2);

console.log(JSON.stringify(out));
"""

with tempfile.NamedTemporaryFile("w", suffix=".mjs", delete=False, dir=str(DT)) as f:
    f.write(JS)
    js_path = f.name
try:
    result = subprocess.run(["node", js_path], capture_output=True, text=True, cwd=str(DT))
    if result.returncode != 0:
        print("JS ERROR:", result.stderr)
        sys.exit(1)
    js = json.loads(result.stdout)
finally:
    os.unlink(js_path)

# <META - ROLE : Execute jset | L46-47>
def jset(key):
    return set(tuple(p) for p in js[key])

# <META - ROLE : Execute pil line | L50-54>
def pil_line(pts, width):
    img = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.line(pts, fill=(255, 0, 0, 255), width=width)
    return set((x, y) for y in range(128) for x in range(128) if img.getpixel((x, y))[3] != 0)

for name, args in [("seg1", [(38, 20), (80, 64)]), ("seg2", [(80, 64), (38, 108)])]:
    pil = pil_line(args, 2)
    js_px = jset(f"wide_line_{name}")
    print(f"wide_line {name}: PIL={len(pil)} JS={len(js_px)} onlyPIL={len(pil-js_px)} onlyJS={len(js_px-pil)}")
    print("  onlyPIL:", sorted(pil - js_px))
    print("  onlyJS:", sorted(js_px - pil))

pil = pil_line([(38, 20), (80, 64), (38, 108)], 2)
print(f"polyline total PIL={len(pil)}")
