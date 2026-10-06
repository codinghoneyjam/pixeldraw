#!/usr/bin/env python3
# <META - FILE SUMMARY - Generate modular cybernetic weapon assets from JSON shapes>
"""Modular Cybernetic Weapon Generator for Player and NPC Armaments.

Renders 128x128 pixel-consistent modular weapons (Sword, Spear, Bow)
from declarative JSON shapes with dedicated RGBA emission masks and sockets.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

try:
    from PIL import Image
except ImportError:
    print("[ERROR] Pillow is required: pip install Pillow")
    sys.exit(1)

from dev.tools.assets.core.geometry import render_layer_commands
from dev.tools.assets.core.paths import SHAPES_WEAPON, src

# <META - ROLE : Return default path to weapon shapes directory | L27-29>
def _default_shapes_dir() -> Path:
    """Return default path to assetdb/entity/weapon."""
    return src(SHAPES_WEAPON)

# <META - ROLE : Load and validate weapon shape JSON specification file. | L32-42>
def load_weapon_shape(shape_path: Path) -> dict[str, Any]:
    if not shape_path.exists():
        raise FileNotFoundError(f"Weapon shape file not found: {shape_path}")
    data: Any = json.loads(shape_path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"Weapon shape root must be a JSON object: {shape_path}")
    required_keys = ("name", "canvas", "metadata", "albedo_layers", "mask_layers")
    for key in required_keys:
        if key not in data:
            raise ValueError(f"Missing required key '{key}' in weapon shape: {shape_path}")
    return data

# <META - ROLE : Render albedo, emission mask, and metadata from declarative shape dictionary. | L45-62>
def render_weapon_from_shape(shape_def: dict[str, Any]) -> tuple[Image.Image, Image.Image, dict[str, Any]]:
    canvas_raw = shape_def.get("canvas", [128, 128])
    canvas_size: tuple[int, int] = (int(canvas_raw[0]), int(canvas_raw[1]))

    albedo = Image.new("RGBA", canvas_size, (0, 0, 0, 0))
    mask = Image.new("RGBA", canvas_size, (0, 0, 0, 0))

    spec: dict[str, Any] = {}
    palette: dict[str, Any] = shape_def.get("palette", {})
    for k, v in palette.items():
        spec[k] = v
        spec[k.lstrip("$")] = v

    render_layer_commands(albedo, shape_def.get("albedo_layers", []), spec)
    render_layer_commands(mask, shape_def.get("mask_layers", []), spec)

    metadata: dict[str, Any] = dict(shape_def.get("metadata", {}))
    return albedo, mask, metadata

# <META - ROLE : Build and save all modular weapon textures and manifest JSON. | L65-87>
def build_weapon_assets(output_dir: Path, shapes_dir: Path | None = None, manifest_path: Path | None = None) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=True)
    active_shapes_dir = shapes_dir if shapes_dir is not None else _default_shapes_dir()

    manifest: dict[str, Any] = {}
    shape_files = sorted(active_shapes_dir.glob("weapon_*.json"))
    for shape_file in shape_files:
        shape_def = load_weapon_shape(shape_file)
        name = str(shape_def["name"])
        albedo_img, mask_img, meta = render_weapon_from_shape(shape_def)
        alb_file = f"{name}_albedo.png"
        msk_file = f"{name}_mask.png"
        albedo_img.save(output_dir / alb_file)
        mask_img.save(output_dir / msk_file)
        item_data = dict(meta)
        item_data["albedo"] = alb_file
        item_data["emission_mask"] = msk_file
        manifest[name] = item_data

    target_manifest: Path = manifest_path if manifest_path is not None else output_dir / "modular_weapons_manifest.json"
    target_manifest.parent.mkdir(parents=True, exist_ok=True)
    target_manifest.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return manifest

# <META - ROLE : Backward-compatible helper to render sword assets. | L90-92>
def render_sword(shapes_dir: Path | None = None) -> tuple[Image.Image, Image.Image, dict[str, Any]]:
    active_dir = shapes_dir if shapes_dir is not None else _default_shapes_dir()
    return render_weapon_from_shape(load_weapon_shape(active_dir / "weapon_sword.json"))

# <META - ROLE : Backward-compatible helper to render spear assets. | L95-97>
def render_spear(shapes_dir: Path | None = None) -> tuple[Image.Image, Image.Image, dict[str, Any]]:
    active_dir = shapes_dir if shapes_dir is not None else _default_shapes_dir()
    return render_weapon_from_shape(load_weapon_shape(active_dir / "weapon_spear.json"))

# <META - ROLE : Backward-compatible helper to render bow assets. | L100-102>
def render_bow(shapes_dir: Path | None = None) -> tuple[Image.Image, Image.Image, dict[str, Any]]:
    active_dir = shapes_dir if shapes_dir is not None else _default_shapes_dir()
    return render_weapon_from_shape(load_weapon_shape(active_dir / "weapon_bow.json"))

# <META - ROLE : CLI entrypoint for batch modular weapon asset generation. | L105-112>
def main() -> None:
    parser = argparse.ArgumentParser(description="Generate cybernetic modular weapon assets")
    parser.add_argument("--output", "-o", type=Path, default=Path("assetdb/entity/weapon"), help="Output directory")
    parser.add_argument("--shapes", "-s", type=Path, default=None, help="Shapes JSON directory")
    parser.add_argument("--manifest", "-m", type=Path, default=Path("assetdb/entity/weapon/modular_weapons_manifest.json"), help="Manifest JSON path")
    args = parser.parse_args()
    manifest = build_weapon_assets(args.output, args.shapes, args.manifest)
    print(f"[GEN] Generated {len(manifest)} modular weapons in {args.output}")

if __name__ == "__main__":
    main()
