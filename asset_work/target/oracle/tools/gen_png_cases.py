# <META - FILE SUMMARY - Project module implementation>
"""Writes tests/fixtures/png_cases.json: PNGs with every filter type (0-4), bad CRC, unsupported color types.
Each valid case: expected RGBA (hex string). PIL is used to cross-verify the hand-built filtered PNGs."""
import base64, io, json, struct, zlib
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]

# <META - ROLE : Execute chunk | L11-19>
def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
# <META - ROLE : Execute png | L0-0>
def png(w, h, ctype, raw_filtered, bad_crc=False, depth=8):
    ihdr = struct.pack(">IIBBBBB", w, h, depth, ctype, 0, 0, 0)
    idat = zlib.compress(raw_filtered)
    out = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")
    if bad_crc:
        i = out.index(b"IDAT") + 4; out = out[:i] + bytes([out[i] ^ 0xFF]) + out[i+1:]
    return out

# <META - ROLE : Execute paeth | L22-24>
def paeth(a, b, c):
    p = a + b - c; pa, pb, pc = abs(p-a), abs(p-b), abs(p-c)
    return a if pa <= pb and pa <= pc else (b if pb <= pc else c)

W, H, BPP = 6, 5, 4
px = bytearray()
for y in range(H):
    for x in range(W):
        px += bytes([(x*40+y*7) % 256, (y*50+x*3) % 256, (x*y*11) % 256, 255 if (x+y) % 3 else 0])
rows = [bytes(px[y*W*4:(y+1)*W*4]) for y in range(H)]
filt = bytearray()
for y, row in enumerate(rows):
    f = y % 5
    prev = rows[y-1] if y else bytes(len(row))
    out = bytearray()
    for i, v in enumerate(row):
        a = row[i-BPP] if i >= BPP else 0; b = prev[i]; c = prev[i-BPP] if i >= BPP else 0
        pred = [0, a, b, (a+b)//2, paeth(a, b, c)][f]
        out.append((v - pred) & 255)
    filt += bytes([f]) + out
good = png(W, H, 6, bytes(filt))
assert list(Image.open(io.BytesIO(good)).convert("RGBA").tobytes()) == list(px), "hand-built PNG must decode identically in PIL"

gray = io.BytesIO(); Image.new("L", (4, 4), 100).save(gray, format="PNG")
rgb = io.BytesIO(); Image.new("RGB", (4, 4), (1, 2, 3)).save(rgb, format="PNG")
inter = io.BytesIO(); Image.new("RGBA", (4, 4), (1, 2, 3, 4)).save(inter, format="PNG", interlace=1) if False else None
b64 = lambda b: base64.b64encode(b).decode()
cases = {"cases": [
  {"name": "all_five_filters_6x5", "png": b64(good), "width": W, "height": H, "expect_error": None, "rgba_hex": bytes(px).hex()},
  {"name": "bad_crc", "png": b64(png(W, H, 6, bytes(filt), bad_crc=True)), "expect_error": "PNG_CRC"},
  {"name": "gray_8bit_unsupported", "png": b64(gray.getvalue()), "expect_error": "PNG_UNSUPPORTED"},
  {"name": "rgb_8bit_unsupported", "png": b64(rgb.getvalue()), "expect_error": "PNG_UNSUPPORTED"},
  {"name": "not_a_png", "png": b64(b"hello world, not a png"), "expect_error": "PNG_SIGNATURE"},
]}
(ROOT / "tests" / "fixtures" / "png_cases.json").write_text(json.dumps(cases, indent=1) + "\n")
print("wrote", len(cases["cases"]), "png cases")
