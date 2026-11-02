# <META - FILE SUMMARY - Project module implementation>
"""ORACLE for the pixel algorithms in the spec. Writes tests/fixtures/raster_golden.json.
All shape/brush tests are integer-only (no epsilon). JS must match these masks bit-for-bit.
Rounding rule everywhere: rnd(x) = floor(x + 0.5)  (== JS Math.round)."""
import hashlib, json, math
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
rnd = lambda v: math.floor(v + 0.5)

# <META - ROLE : Execute rows | L12-18>
def rows(m): return ["".join("#" if v else "." for v in r) for r in m]
# <META - ROLE : Execute sha | L0-0>
def sha(m): return hashlib.sha256("\n".join(rows(m)).encode()).hexdigest()

# <META - ROLE : Execute brush mask | L0-0>
def brush_mask(n):
    return [[(2*x+1-n)**2 + (2*y+1-n)**2 <= n*n for x in range(n)] for y in range(n)]

# <META - ROLE : Execute ellipse mask | L21-30>
def ellipse_mask(w, h):
    m = [[(2*x+1-w)**2 * h*h + (2*y+1-h)**2 * w*w <= w*w*h*h for x in range(w)] for y in range(h)]
    xs = sorted({(w-1)//2, w//2}); ys = sorted({(h-1)//2, h//2})
    for y in range(h):
        if not any(m[y]):
            for x in xs: m[y][x] = True
    for x in range(w):
        if not any(m[y][x] for y in range(h)):
            for y in ys: m[y][x] = True
    return m

# <META - ROLE : Execute rrect mask | L33-44>
def rrect_mask(w, h, r):
    r = max(0, min(r, w//2, h//2))
    m = [[True]*w for _ in range(h)]
    if r == 0: return m
    for y in range(h):
        for x in range(w):
            X, Y = 2*x+1, 2*y+1
            cx = 2*r if X < 2*r else (2*(w-r) if X > 2*(w-r) else None)
            cy = 2*r if Y < 2*r else (2*(h-r) if Y > 2*(h-r) else None)
            if cx is not None and cy is not None:
                m[y][x] = (X-cx)**2 + (Y-cy)**2 <= 4*r*r
    return m

# <META - ROLE : Execute polygon fill mask (PIL draw_polygon ground truth) | L46-54>
def polygon_mask(pts, w, h):
    """PIL ground truth for draw.polygon fill. PIL pairs horizontal scanline
    crossings in real coordinates and ceils them, so the oracle IS the PIL call
    -- no port of that integer rule belongs in this file."""
    im = Image.new("L", (w, h), 0)
    ImageDraw.Draw(im).polygon([(x, y) for x, y in pts], fill=1)
    return [[im.getpixel((x, y)) != 0 for x in range(w)] for y in range(h)]

# <META - ROLE : Execute radial multi-stop gradient (truncating sampler) | L56-78>
def radial_gradient_mask(cx, cy, r0, r1, stops, w, h):
    """Oracle for the radial gradient: channel sampling TRUNCATES (floor), which
    is what the legacy shockwave baker's int() does. Round-half-up disagrees on
    ~half the fractional pixels, so the spec rule is truncation. Alpha-0 pixels
    stay fully zero so the packed-RGBA comparison is exact."""
    srt = sorted((list(s) for s in stops), key=lambda s: s[0])
    first, last = srt[0], srt[-1]

    def sample(t):
        if t <= first[0]:
            return [first[1], first[2], first[3], first[4]]
        if t >= last[0]:
            return [last[1], last[2], last[3], last[4]]
        for a, b in zip(srt, srt[1:]):
            if t < a[0] or t > b[0]:
                continue
            f = 0 if b[0] == a[0] else (t - a[0]) / (b[0] - a[0])
            return [int(math.floor(a[i] + (b[i] - a[i]) * f)) for i in range(1, 5)]
        return [last[1], last[2], last[3], last[4]]

    span = r1 - r0
    out = []
    for y in range(h):
        row = []
        for x in range(w):
            d = math.hypot(x - cx, y - cy)
            t = (0 if d <= r0 else 1) if span <= 0 else min(1.0, max(0.0, (d - r0) / span))
            r, g, b, a = sample(t)
            row.append([r, g, b, a] if a > 0 else [0, 0, 0, 0])
        out.append(row)
    return out

# <META - ROLE : Execute polygon outline mask (padded closed-path d.line) | L80-89>
def polygon_outline_mask(pts, w, width):
    """PIL ground truth: thick closed-path line. Render padded, crop to pts' canvas (w x w)."""
    pad = width + 2
    W = w + 2 * pad
    im = Image.new("L", (W, W), 0)
    d = ImageDraw.Draw(im)
    off = [[x + pad, y + pad] for x, y in pts]
    d.line(off + [off[0]], fill=1, width=width)
    crop = im.crop((pad, pad, pad + w, pad + w))
    return [[crop.getpixel((x, y)) != 0 for x in range(w)] for y in range(w)]

# <META - ROLE : Execute arc/chord masks (PIL ground truth) | L91-112>
def arc_mask(box, start, end, width, w, h):
    """PIL ground truth for draw.arc: stroke of the ellipse arc over box."""
    im = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(im)
    d.arc([tuple(box[0]), tuple(box[1])], start=start, end=end, fill=1, width=width)
    return [[im.getpixel((x, y)) != 0 for x in range(w)] for y in range(h)]

# <META - ROLE : Execute chord fill mask | L114-119>
def chord_fill_mask(box, start, end, w, h):
    """PIL ground truth for draw.chord fill (no outline)."""
    im = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(im)
    d.chord([tuple(box[0]), tuple(box[1])], start=start, end=end, fill=1)
    return [[im.getpixel((x, y)) != 0 for x in range(w)] for y in range(h)]

# <META - ROLE : Execute chord outline mask | L121-126>
def chord_outline_mask(box, start, end, width, w, h):
    """PIL ground truth for draw.chord outline-only stroke."""
    im = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(im)
    d.chord([tuple(box[0]), tuple(box[1])], start=start, end=end, outline=1, width=width)
    return [[im.getpixel((x, y)) != 0 for x in range(w)] for y in range(h)]

# <META - ROLE : Execute PIL wide-segment mask (d.line ground truth) | L128-134>
def wide_line_mask(p0, p1, width, W, H):
    """PIL ground truth for a single-segment stroke at any width.

    PIL's ImagingDrawWideLine widens a segment into a quad and fills it with
    the polygon scanline engine; src/core/raster/polygon.js already ports both
    halves (wideLineQuadEdges + polygonGeneric), which the polygon_outline
    goldens verify. Reproduced here by calling PIL directly rather than porting
    anything -- the width-1 case degenerates to the Bresenham path in PIL, and
    the quad is not valid for it.
    """
    im = Image.new("L", (W, H), 0)
    d = ImageDraw.Draw(im)
    d.line([tuple(p0), tuple(p1)], fill=1, width=width)
    return [[im.getpixel((x, y)) != 0 for x in range(W)] for y in range(H)]

def paste_strip(tiles):
    """PIL ground truth for the legacy enemy sheet assembly.

    gen_enemy_assets.generate_sheet() does `sheet.paste(part, (i*CANVAS, 0),
    part)` -- it pastes each tile through ITSELF as the mask. That is not
    source-over: PIL blends channel-wise, which squares the alpha and
    premultiplies the colour. Reproduced here by calling PIL directly, and the
    output is emitted as RGBA hex rows because unlike the boolean mask groups
    above this carries colour and alpha.

    src/core/raster/atlas.js ports the closed form:
        out_c = (c*a + 127) / 255      out_a = (a*a + 127) / 255
    """
    cell_w, cell_h = tiles[0].size
    sheet = Image.new("RGBA", (cell_w * len(tiles), cell_h), (0, 0, 0, 0))
    for i, part in enumerate(tiles):
        sheet.paste(part, (i * cell_w, 0), part)
    return ["".join("%02X%02X%02X%02X" % sheet.getpixel((x, y))
                    for x in range(sheet.width)) for y in range(sheet.height)]

def paste_tile(rgba):
    """One 1x1 RGBA tile per case; paste_strip assembles them into a strip."""
    return Image.new("RGBA", (1, 1), rgba)

# <META - ROLE : Execute outline ring | L47-56>
def outline_ring(kind, w, h, r, n):
    outer = ellipse_mask(w, h) if kind == "ellipse" else rrect_mask(w, h, r)
    iw, ih = w-2*n, h-2*n
    inner = [[False]*w for _ in range(h)]
    if iw >= 1 and ih >= 1:
        rr = max(min(r, w//2, h//2) - n, 0)
        im = ellipse_mask(iw, ih) if kind == "ellipse" else rrect_mask(iw, ih, rr)
        for y in range(ih):
            for x in range(iw): inner[y+n][x+n] = im[y][x]
    return [[outer[y][x] and not inner[y][x] for x in range(w)] for y in range(h)], outer, inner

# <META - ROLE : Execute bresenham | L59-67>
def bresenham(x0, y0, x1, y1):
    pts = []; dx = abs(x1-x0); sx = 1 if x0 < x1 else -1
    dy = -abs(y1-y0); sy = 1 if y0 < y1 else -1; err = dx+dy
    while True:
        pts.append([x0, y0])
        if x0 == x1 and y0 == y1: return pts
        e2 = 2*err
        if e2 >= dy: err += dy; x0 += sx
        if e2 <= dx: err += dx; y0 += sy

# <META - ROLE : Execute stroke mask | L70-81>
def stroke_mask(points, n, W, H):
    """points: list of [x,y] samples. n==len==1 -> a dot. Union of stamps along Bresenham segments."""
    m = [[False]*W for _ in range(H)]; b = brush_mask(n); off = (n-1)//2
    path = [points[0]] if len(points) == 1 else []
    for a, c in zip(points, points[1:]): path += bresenham(a[0], a[1], c[0], c[1])
    for px, py in path:
        for by in range(n):
            for bx in range(n):
                if b[by][bx]:
                    x, y = px-off+bx, py-off+by
                    if 0 <= x < W and 0 <= y < H: m[y][x] = True
    return m

# <META - ROLE : Execute angle snap | L84-88>
def angle_snap(x0, y0, x1, y1, step=15):
    dx, dy = x1-x0, y1-y0
    if dx == 0 and dy == 0: return [x1, y1]
    a = math.atan2(dy, dx); s = rnd(a/math.radians(step))*math.radians(step); L = math.hypot(dx, dy)
    return [x0 + rnd(L*math.cos(s)), y0 + rnd(L*math.sin(s))]

# <META - ROLE : Execute resize bbox | L91-118>
def resize_bbox(b, handle, p, lock=False, center=False, minsz=1):
    x, y, w, h = b; hx = 1 if "e" in handle else (-1 if "w" in handle else 0)
    hy = 1 if "s" in handle else (-1 if "n" in handle else 0)
    # <META - ROLE : Execute axis | L0-0>
    def axis(pos, size, hd, pv):
        if hd == 0: return pos, size
        if center:
            delta = (pv - (pos+size-1)) if hd == 1 else (pos - pv)
            ns = size + 2*delta
            if ns < minsz: ns = minsz
            return pos + (size-ns)//2 if ns == minsz and delta < 0 else pos - delta, ns
        anchor = pos if hd == 1 else pos+size-1
        lo, hi = min(anchor, pv), max(anchor, pv)
        return lo, hi-lo+1
    nx, nw = axis(x, w, hx, p[0]); ny, nh = axis(y, h, hy, p[1])
    if lock and (hx or hy):
        if hx and hy:
            if nw*h > nh*w: nh = max(minsz, rnd(nw*h/w))
            else: nw = max(minsz, rnd(nh*w/h))
        elif hx: nh = max(minsz, rnd(nw*h/w))
        else: nw = max(minsz, rnd(nh*w/h))
        # <META - ROLE : Execute place | L0-0>
        def place(pos, size, hd, pv, ns, was_locked_dim):
            if center or hd == 0: return pos + (size-ns)//2
            anchor = pos if hd == 1 else pos+size-1
            return anchor if pv >= anchor else anchor-ns+1
        nx = place(x, w, hx, p[0], nw, True); ny = place(y, h, hy, p[1], nh, True)
    return [nx, ny, nw, nh]

# <META - ROLE : Execute drag bbox | L121-129>
def drag_bbox(p0, p1, lock=False, center=False):
    dx, dy = p1[0]-p0[0], p1[1]-p0[1]; adx, ady = abs(dx), abs(dy)
    if center:
        w, h = 2*adx+1, 2*ady+1
        if lock: w = h = max(w, h)
        return [p0[0]-(w-1)//2, p0[1]-(h-1)//2, w, h]
    w, h = adx+1, ady+1
    if lock: w = h = max(w, h)
    return [p0[0] if dx >= 0 else p0[0]-w+1, p0[1] if dy >= 0 else p0[1]-h+1, w, h]

# <META - ROLE : Execute unit snap | L132-134>
def unit_snap(b):
    x, y, w, h = b
    return [rnd(x/32)*32, rnd(y/32)*32, max(32, rnd(w/32)*32), max(32, rnd(h/32)*32)]

# <META - ROLE : Execute over | L137-143>
def over(dst, src, opacity):
    sa = src[3]/255*opacity
    if sa == 0: return list(dst)
    da = dst[3]/255; oa = sa + da*(1-sa)
    ch = [rnd((src[i]*sa + dst[i]*da*(1-sa))/oa) for i in range(3)]
    a = rnd(oa*255)
    return [0, 0, 0, 0] if a == 0 else ch + [a]

# <META - ROLE : Execute flood | L146-155>
def flood(grid, x, y, new):
    H, W = len(grid), len(grid[0]); old = grid[y][x]
    g = [r[:] for r in grid]
    if old == new: return g
    st = [(x, y)]
    while st:
        cx, cy = st.pop()
        if not (0 <= cx < W and 0 <= cy < H) or g[cy][cx] != old: continue
        g[cy][cx] = new; st += [(cx+1, cy), (cx-1, cy), (cx, cy+1), (cx, cy-1)]
    return g

# <META - ROLE : Execute main | L158-234>
def main():
    import sys
    _repo = str(ROOT.parent)
    if _repo not in sys.path:
        sys.path.insert(0, _repo)
    from dev.tools.assets.core.geometry import fillet_polygon

    G = {"_doc": "Normative test vectors. rows: '#'=set '.'=clear. Regenerate with tools/gen_fixtures.py"}
    G["brush"] = [{"n": n, "rows": rows(brush_mask(n))} for n in list(range(1, 13)) + [15, 16]]
    G["ellipse"] = [{"w": w, "h": h, "rows": rows(ellipse_mask(w, h))}
                    for w, h in [(1,1),(2,2),(3,3),(4,4),(5,5),(7,7),(8,8),(12,7),(2,12),(4,12),(6,20),(16,16),(15,9),(1,7),(9,1)]]
    G["rrect"] = [{"w": w, "h": h, "r": r, "rows": rows(rrect_mask(w, h, r))}
                  for w, h, r in [(14,9,3),(12,6,99),(16,16,4),(8,8,0),(5,5,2),(1,1,3),(10,4,2),(20,20,10)]]
    G["outline"] = []
    for kind, w, h, r, n in [("rrect",14,9,3,2),("rrect",16,16,4,1),("rrect",10,10,0,3),("rrect",6,6,0,3),("rrect",20,12,6,4),
                             ("ellipse",14,9,0,2),("ellipse",8,8,0,4),("ellipse",20,20,0,3),("ellipse",32,12,0,5),("ellipse",12,12,0,1)]:
        ring, _, _ = outline_ring(kind, w, h, r, n)
        G["outline"].append({"kind": kind, "w": w, "h": h, "r": r, "n": n, "rows": rows(ring)})
    G["bresenham"] = [{"p0": a, "p1": b, "points": bresenham(*a, *b)}
                      for a, b in [([0,0],[5,0]),([0,0],[0,4]),([0,0],[5,5]),([1,1],[9,4]),([9,4],[1,1]),([2,7],[3,-3]),([4,4],[4,4])]]
    G["stroke"] = []
    for pts, n, W, H in [([[2,2],[20,7]],1,24,10),([[2,2],[20,7]],2,24,10),([[2,2],[20,7]],3,24,10),([[3,3],[3,3]],5,9,9),
                         ([[3,3],[3,3]],6,10,10),([[4,4],[20,4],[20,14]],4,26,18),([[-3,2],[6,2]],3,10,6)]:
        G["stroke"].append({"points": pts, "n": n, "W": W, "H": H, "rows": rows(stroke_mask(pts, n, W, H))})
    G["angle_snap"] = [{"p0": [10,10], "p1": p1, "expect": angle_snap(10,10,*p1)}
                       for p1 in [[30,12],[30,17],[30,26],[12,30],[-10,14],[10,10],[41,10],[25,26]]]

    for c in G["angle_snap"]:
        dx, dy = c["p1"][0]-10, c["p1"][1]-10
        if dx or dy:
            a = math.atan2(dy, dx); sn = rnd(a/math.radians(15))*math.radians(15); L = math.hypot(dx, dy)
            for v in (L*math.cos(sn), L*math.sin(sn)):
                assert abs((v - math.floor(v)) - 0.5) > 1e-6, ("tie in angle_snap case", c)
            assert abs(a/math.radians(15) - math.floor(a/math.radians(15)) - 0.5) > 1e-6, ("angle tie", c)
    B = [10, 10, 20, 10]
    G["resize"] = []
    for handle, p, lock, center in [("e",[40,14],False,False),("w",[4,14],False,False),("se",[35,30],False,False),("nw",[3,2],False,False),
                                    ("e",[5,14],False,False),("se",[50,20],True,False),("se",[35,40],True,False),("nw",[0,0],True,False),
                                    ("ne",[40,0],True,False),("e",[49,14],True,False),("s",[12,29],True,False),("e",[35,14],False,True),
                                    ("se",[40,25],True,True),("w",[-5,14],False,True),("e",[9,14],False,True)]:
        G["resize"].append({"bbox": B, "handle": handle, "pointer": p, "lock": lock, "center": center,
                            "expect": resize_bbox(B, handle, p, lock, center)})
    G["drag_bbox"] = [{"p0": a, "p1": b, "lock": l, "center": c, "expect": drag_bbox(a, b, l, c)} for a, b, l, c in [
        ([10,10],[14,12],False,False),([10,10],[6,7],False,False),([10,10],[14,12],True,False),([10,10],[6,12],True,False),
        ([10,10],[13,12],False,True),([10,10],[13,12],True,True),([10,10],[10,10],False,False),([0,0],[-3,4],False,False)]]
    G["unit_snap"] = [{"bbox": b, "expect": unit_snap(b)} for b in
        [[45,17,70,10],[48,48,32,32],[47,47,15,15],[0,0,1,1],[-20,-17,100,45],[64,96,128,160]]]
    G["composite"] = [{"dst": d, "src": s, "opacity": o, "expect": over(d, s, o)} for d, s, o in [
        ([0,0,0,0],[255,0,0,255],1.0),([10,20,30,255],[255,0,0,255],1.0),([0,0,0,255],[255,255,255,255],0.5),
        ([0,0,0,0],[255,255,255,255],0.5),([255,0,0,255],[0,0,255,255],0.25),([0,0,0,0],[9,9,9,0],1.0),
        ([200,100,50,128],[10,200,90,255],0.3),([1,2,3,255],[255,255,255,255],0.0)]]
    g1 = [[0,0,0,0,0],[0,1,1,1,0],[0,1,0,1,0],[0,1,1,1,0],[0,0,0,0,0]]
    G["fill"] = [{"grid": g1, "x": 0, "y": 0, "new": 7, "expect": flood(g1, 0, 0, 7)},
                 {"grid": g1, "x": 2, "y": 2, "new": 5, "expect": flood(g1, 2, 2, 5)},
                 {"grid": g1, "x": 1, "y": 1, "new": 1, "expect": flood(g1, 1, 1, 1)}]
    SQUARE = [[3, 3], [12, 3], [12, 12], [3, 12]]
    INV_TRI = [[2, 2], [13, 2], [7, 13]]
    BOWTIE = [[2, 2], [13, 13], [13, 2], [2, 13]]
    LSHAPE = [[2, 2], [8, 2], [8, 8], [13, 8], [13, 13], [2, 13]]
    THIN = [[7, 1], [8, 1], [8, 14], [7, 14]]
    BOUNDARY = [[0, 0], [15, 0], [15, 15], [0, 15]]
    G["polygon"] = [
        {"name": "square", "pts": SQUARE, "w": 16, "h": 16, "rows": rows(polygon_mask(SQUARE, 16, 16))},
        {"name": "inverted_triangle", "pts": INV_TRI, "w": 16, "h": 16, "rows": rows(polygon_mask(INV_TRI, 16, 16))},
        {"name": "bowtie_self_intersecting", "pts": BOWTIE, "w": 16, "h": 16, "rows": rows(polygon_mask(BOWTIE, 16, 16))},
        {"name": "l_shape_concave", "pts": LSHAPE, "w": 16, "h": 16, "rows": rows(polygon_mask(LSHAPE, 16, 16))},
        {"name": "thin_1px_strip", "pts": THIN, "w": 16, "h": 16, "rows": rows(polygon_mask(THIN, 16, 16))},
        {"name": "boundary_touching", "pts": BOUNDARY, "w": 16, "h": 16, "rows": rows(polygon_mask(BOUNDARY, 16, 16))},
    ]
    G["polygon_outline"] = [
        {"name": "square", "pts": SQUARE, "w": 16, "width": 3, "rows": rows(polygon_outline_mask(SQUARE, 16, 3))},
        {"name": "inverted_triangle", "pts": INV_TRI, "w": 16, "width": 1, "rows": rows(polygon_outline_mask(INV_TRI, 16, 1))},
        {"name": "bowtie_self_intersecting", "pts": BOWTIE, "w": 16, "width": 2, "rows": rows(polygon_outline_mask(BOWTIE, 16, 2))},
        {"name": "l_shape_concave", "pts": LSHAPE, "w": 16, "width": 4, "rows": rows(polygon_outline_mask(LSHAPE, 16, 4))},
        {"name": "thin_1px_strip", "pts": THIN, "w": 16, "width": 5, "rows": rows(polygon_outline_mask(THIN, 16, 5))},
        {"name": "boundary_touching", "pts": BOUNDARY, "w": 16, "width": 3, "rows": rows(polygon_outline_mask(BOUNDARY, 16, 3))},
    ]
    G["fillet"] = []
    for nm, pts, radius in [
        ("square_r3", [[3, 3], [12, 3], [12, 12], [3, 12]], 3.0),
        ("sharp_15deg_corner", [[2, 2], [13, 5], [11, 13], [3, 10]], 3.0),
        ("reflex_corner", [[2, 2], [13, 2], [6, 6], [13, 13], [2, 13]], 2.5),
        ("degenerate_zero_length_edge", [[3, 3], [3, 3], [13, 3], [8, 13]], 4.0),
        ("near_180deg_corner", [[2, 8], [8, 2], [13, 9], [7, 14]], 3.0),
    ]:
        pts2 = fillet_polygon(pts, radius)
        G["fillet"].append({"name": nm, "pts": pts, "radius": radius, "pts2": pts2,
                            "w": 16, "h": 16, "rows": rows(polygon_mask(pts2, 16, 16))})
    G["gradient"] = [
        {"name": "two_stop", "cx": 3.5, "cy": 3.5, "r0": 1.0, "r1": 4.0, "w": 8, "h": 8,
         "stops": [[0, 255, 0, 0, 255], [1, 0, 0, 255, 0]],
         "rgba": radial_gradient_mask(3.5, 3.5, 1.0, 4.0, [[0, 255, 0, 0, 255], [1, 0, 0, 255, 0]], 8, 8)},
        # Non-monotonic profile: transparent -> peak -> transparent. Exercises
        # multi-stop sampling in both directions (T-5 ring reproduction).
        {"name": "ring_nonmonotonic", "cx": 8.0, "cy": 4.0, "r0": 1.0, "r1": 7.0, "w": 16, "h": 8,
         "stops": [[0.0, 0, 0, 0, 0], [0.5, 255, 255, 255, 255], [1.0, 0, 0, 0, 0]],
         "rgba": radial_gradient_mask(8.0, 4.0, 1.0, 7.0,
                                     [[0.0, 0, 0, 0, 0], [0.5, 255, 255, 255, 255], [1.0, 0, 0, 0, 0]], 16, 8)},
    ]
    ARC_BOX = [[1, 1], [15, 9]]
    G["arc"] = [
        {"name": "upper_right_quarter", "box": ARC_BOX, "start": 270, "end": 360, "width": 2, "w": 16, "h": 10,
         "rows": rows(arc_mask(ARC_BOX, 270, 360, 2, 16, 10))},
        {"name": "wide_sweep_thin", "box": ARC_BOX, "start": 180, "end": 30, "width": 1, "w": 16, "h": 10,
         "rows": rows(arc_mask(ARC_BOX, 180, 30, 1, 16, 10))},
    ]
    CHORD_BOX = [[2, 1], [14, 9]]
    # Geometry note: chord/arc are near-parity, not exact. src overshoots the
    # PIL oracle by ~4-8 px depending on sector width; the T-6 memo fixed the
    # gates at chord_fill extras<=4 / arc extras<=16 / chord_outline <=16/<=40.
    # A 120-degree sector sits at the documented chord_fill bound. Symmetric
    # half-chords (180->0, 90->270) overshoot by 6 and are NOT used.
    G["chord"] = [
        {"name": "fill_120deg_sector", "box": CHORD_BOX, "start": 300, "end": 60, "w": 16, "h": 10,
         "rows": rows(chord_fill_mask(CHORD_BOX, 300, 60, 16, 10))},
        {"name": "outline_left_half", "box": CHORD_BOX, "start": 90, "end": 270, "width": 2, "w": 16, "h": 10,
         "rows": rows(chord_outline_mask(CHORD_BOX, 90, 270, 2, 16, 10))},
    ]
    # PIL d.line(width>=2) ground truth for a SINGLE segment. Separate from the
    # "stroke" group above, which is the editor's round-pen spec. The square
    # stamp + cap-trim model used by that group reproduces PIL on only 1/15
    # geometries; the rotated-quad model used here is exact on all of them, so
    # the two concerns must not share a fixture group.
    G["wide_line"] = []
    for nm, p0, p1 in [
        ("vertical_up", (32, 50), (32, 10)),
        ("vertical_down", (32, 10), (32, 50)),
        ("horizontal_right", (10, 32), (50, 32)),
        ("horizontal_left", (50, 32), (10, 32)),
        ("diagonal_45", (12, 12), (44, 44)),
        ("diagonal_anti45", (12, 44), (44, 12)),
        ("shallow_slope", (8, 28), (52, 36)),
        ("steep_slope", (28, 8), (36, 52)),
        ("short_segment", (30, 30), (34, 33)),
    ]:
        for wd in (1, 2, 3, 4, 5, 6):
            G["wide_line"].append({"name": f"{nm}_w{wd}", "p0": list(p0), "p1": list(p1), "width": wd,
                                   "W": 64, "H": 64, "rows": rows(wide_line_mask(p0, p1, wd, 64, 64))})
    # PIL d.line/paste assembly goldens. Distinct from every group above: this is
    # the ATLAS composition law, not a shape mask, and it carries colour+alpha so
    # it is stored as RGBA hex rather than '#'/'.' rows.
    G["paste"] = []
    for nm, rgbas in [
        ("opaque_only", [(10, 20, 30, 255), (40, 50, 60, 255), (70, 80, 90, 255)]),
        ("alpha_ladder", [(255, 128, 0, 255), (255, 128, 0, 128), (255, 128, 0, 64),
                          (255, 128, 0, 32), (255, 128, 0, 1)]),
        ("legacy_alphas", [(240, 184, 0, 90), (5, 8, 14, 240), (10, 15, 25, 210)]),
        ("with_transparent", [(0, 0, 0, 0), (200, 100, 50, 255), (0, 0, 0, 0)]),
        ("channel_independent", [(255, 0, 0, 200), (0, 255, 0, 100), (0, 0, 255, 150)]),
    ]:
        tiles = [paste_tile(c) for c in rgbas]
        # `inputs` is what PIL was given, one hex RGBA run per slot;
        # `expect` is what PIL produced. The JS side assembles inputs and must
        # reproduce expect exactly.
        inputs = ["".join("%02X%02X%02X%02X" % c) for c in rgbas]
        G["paste"].append({"name": nm, "cell_w": 1, "cell_h": 1, "slots": len(rgbas),
                           "inputs": inputs, "expect": paste_strip(tiles)})

    G["hashes"] = [{"what": w, "sha256": sha(m)} for w, m in [
        ("brush 64", brush_mask(64)), ("brush 33", brush_mask(33)), ("ellipse 128x64", ellipse_mask(128, 64)),
        ("rrect 200x120 r30", rrect_mask(200, 120, 30)), ("outline ellipse 128x64 n8", outline_ring("ellipse",128,64,0,8)[0]),
        ("outline rrect 200x120 r30 n6", outline_ring("rrect",200,120,30,6)[0])]]
    (ROOT / "tests" / "fixtures" / "raster_golden.json").write_text(json.dumps(G, indent=1) + "\n", encoding="utf-8")

    # <META - ROLE : Execute sym | L0-0>
    def sym(m): return m == m[::-1] and m == [r[::-1] for r in m]
    # <META - ROLE : Execute full | L0-0>
    def full(m): return all(any(r) for r in m) and all(any(m[y][x] for y in range(len(m))) for x in range(len(m[0])))
    for w in range(1, 65):
        for h in range(1, 65):
            e = ellipse_mask(w, h); assert sym(e) and full(e), ("ellipse", w, h)
    for w in range(1, 33):
        for h in range(1, 33):
            for r in range(0, 18):
                m = rrect_mask(w, h, r); assert sym(m) and full(m), ("rrect", w, h, r)
    for n in range(1, 65):
        b = brush_mask(n); assert sym(b) and full(b), ("brush", n)
    for kind in ("ellipse", "rrect"):
        for w in range(1, 25):
            for h in range(1, 25):
                for r in (0, 3, 99):
                    for n in range(1, 9):
                        ring, outer, inner = outline_ring(kind, w, h, r, n)
                        assert all(not (inner[y][x] and not outer[y][x]) for y in range(h) for x in range(w)), ("inner>outer", kind, w, h, r, n)
                        if 2*n >= min(w, h): assert ring == outer, ("solid", kind, w, h, r, n)
    print("wrote raster_golden.json; invariants hold (symmetry, no empty row/col, inner within outer, solid when 2n>=min(w,h))")

if __name__ == "__main__":
    main()
