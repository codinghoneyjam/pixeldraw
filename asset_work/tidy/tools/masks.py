#!/usr/bin/env python3
# <META - FILE SUMMARY - Grayscale mask generation for UI surfaces (tint, specular, emissive)>
"""
Grayscale mask generation for UI surfaces.
tools/assets/core/masks.py

surface RGBA 이미지로부터 tint / specular / emissive grayscale mask를 파생한다.
- tint:     알파가 있는 픽셀의 실루엣(원본 알파를 그대로 grayscale L로 옮김) → 런타임 modulate 용
- specular: 밝기(luminance) 기반 하이라이트 강조 mask → 금속 반사 표현
- emissive: 네온 림/발광 후보(고채도·고명도) 픽셀만 남긴 mask → glow 합성 용

모든 mask는 원본과 동일 크기의 8-bit L 이미지다.
"""
from __future__ import annotations

from PIL import Image

MaskKind = str
SUPPORTED_KINDS: tuple[str, ...] = ("tint", "specular", "emissive")

# <META - ROLE : Compute Rec. 601 approximate luminance from RGB | L22-24>
def _luminance(r: int, g: int, b: int) -> int:
    """Rec. 601 근사 휘도 (0-255)."""
    return (r * 299 + g * 587 + b * 114) // 1000

# <META - ROLE : Create a grayscale mask from the alpha channel | L27-29>
def build_tint_mask(surface: Image.Image) -> Image.Image:
    """원본 알파 채널을 그대로 L 마스크로 사용 (실루엣)."""
    return surface.convert("RGBA").getchannel("A")

# <META - ROLE : Create a highlight mask from bright opaque pixels | L32-47>
def build_specular_mask(surface: Image.Image, threshold: int = 150) -> Image.Image:
    """휘도가 threshold 이상인 불투명 픽셀만 밝기값으로 남긴 하이라이트 마스크."""
    rgba = surface.convert("RGBA")
    width, height = rgba.size
    mask = Image.new("L", (width, height), 0)
    src = rgba.load()
    dst = mask.load()
    for y in range(height):
        for x in range(width):
            r, g, b, a = src[x, y]
            if a == 0:
                continue
            lum = _luminance(r, g, b)
            if lum >= threshold:
                dst[x, y] = lum
    return mask

# <META - ROLE : Create a mask of high-saturation high-luminance neon pixels | L50-66>
def build_emissive_mask(surface: Image.Image, sat_threshold: int = 60, lum_threshold: int = 90) -> Image.Image:
    """채도(max-min)와 휘도가 모두 높은 네온 발광 후보 픽셀을 남긴 마스크."""
    rgba = surface.convert("RGBA")
    width, height = rgba.size
    mask = Image.new("L", (width, height), 0)
    src = rgba.load()
    dst = mask.load()
    for y in range(height):
        for x in range(width):
            r, g, b, a = src[x, y]
            if a == 0:
                continue
            sat = max(r, g, b) - min(r, g, b)
            lum = _luminance(r, g, b)
            if sat >= sat_threshold and lum >= lum_threshold:
                dst[x, y] = min(255, int(a * (sat / 255.0)))
    return mask

_BUILDERS = {
    "tint": build_tint_mask,
    "specular": build_specular_mask,
    "emissive": build_emissive_mask,
}

# <META - ROLE : Dispatch to the appropriate mask builder by kind | L75-80>
def build_mask(surface: Image.Image, kind: MaskKind) -> Image.Image:
    """kind에 해당하는 마스크를 생성한다. 미지원 kind는 ValueError."""
    builder = _BUILDERS.get(kind)
    if builder is None:
        raise ValueError(f"지원하지 않는 mask kind: {kind!r} (지원: {SUPPORTED_KINDS})")
    return builder(surface)

# <META - ROLE : Build a dictionary of masks for requested kinds | L83-85>
def build_masks(surface: Image.Image, kinds: list[str]) -> dict[str, Image.Image]:
    """요청된 kind 목록에 대한 마스크 딕셔너리를 생성한다."""
    return {kind: build_mask(surface, kind) for kind in kinds}
