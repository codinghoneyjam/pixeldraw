#!/usr/bin/env python3
# <META - FILE SUMMARY - Generate speedster shockwave ring texture for boost FX>
"""
Speedster FX Shockwave Ring Texture Generator
tools/assets/generators/gen_speedster_assets.py

Generates high-resolution (256x256) grayscale/alpha shockwave rings for
speedster boost sonic booms. Designed to be modulated by Godot Sprite2D
with player palette colors (Cyan, Gold, etc.) under Additive Blending.
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image

CANVAS_SIZE: int = 256
OUTPUT_DIR: Path = Path("features/entity/player/assets/textures/particles")
OUTPUT_FILE: Path = OUTPUT_DIR / "shockwave_ring.png"

# <META - ROLE : Generate shockwave ring with crisp rim and soft falloff | L23-63>
def generate_shockwave_ring(size: int = CANVAS_SIZE) -> Image.Image:
    """
    Generates a crisp outer rim with a soft inner radial falloff shockwave ring.
    Uses pure grayscale intensity and smooth alpha curve.
    """
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    center = size / 2.0

    r_outer = size * 0.46
    r_peak = size * 0.40
    r_inner = size * 0.22

    pixels = img.load()
    if pixels is None:
        return img

    for y in range(size):
        dy = y - center
        for x in range(size):
            dx = x - center
            dist = math.sqrt(dx * dx + dy * dy)

            if dist > r_outer or dist < r_inner:
                continue

            if dist >= r_peak:

                factor = (r_outer - dist) / (r_outer - r_peak)
                intensity = factor ** 1.8
            else:

                factor = (dist - r_inner) / (r_peak - r_inner)
                intensity = factor ** 2.5

            alpha = int(clamp(intensity * 255.0, 0.0, 255.0))

            lum = int(clamp(200.0 + intensity * 55.0, 0.0, 255.0))

            pixels[x, y] = (lum, lum, lum, alpha)

    return img

# <META - ROLE : Clamp value between minimum and maximum bounds | L66-67>
def clamp(value: float, min_val: float, max_val: float) -> float:
    return max(min_val, min(max_val, value))

# <META - ROLE : CLI entrypoint for shockwave ring generation | L70-74>
def main() -> None:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    ring_img = generate_shockwave_ring(CANVAS_SIZE)
    ring_img.save(OUTPUT_FILE, "PNG")
    print(f"[GEN] Generated shockwave ring: {OUTPUT_FILE} ({CANVAS_SIZE}x{CANVAS_SIZE})")

if __name__ == "__main__":
    main()
