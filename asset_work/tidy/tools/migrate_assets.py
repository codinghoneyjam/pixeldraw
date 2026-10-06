#!/usr/bin/env python3
# <META - FILE SUMMARY - One-shot asset migration utility extracting PNG tilesets from tres and copying assets to assetdb>
"""Asset migration script extracting pure PNGs from inline tres and migrating physical files."""

from __future__ import annotations

import os
from pathlib import Path
import re
import shutil
from PIL import Image

DUPLICATE_WEAPON_NAMES: set[str] = {
    "sword.png",
    "axe.png",
    "bow.png",
    "gun.png",
    "fire_sword.png",
}

# <META - ROLE : Copies matching files by extension from src to dst directory | L22-31>
def _copy_files(src_dir: Path, dst_dir: Path, extensions: tuple[str, ...], exclude: set[str] | None = None) -> None:
    """Copy files matching extensions from src_dir to dst_dir."""
    if not src_dir.exists():
        return
    dst_dir.mkdir(parents=True, exist_ok=True)
    exclude_names: set[str] = exclude if exclude is not None else set()
    for item in src_dir.iterdir():
        if item.is_file() and item.name not in exclude_names:
            if item.suffix.lower() in extensions:
                shutil.copy2(item, dst_dir / item.name)

# <META - ROLE : Extracts RGBA8 PNG image from inline PackedByteArray in a Godot tres file | L34-52>
def extract_png_from_tres(tres_path: Path, output_png_path: Path, width: int, height: int) -> None:
    """Parse PackedByteArray from tres and write output PNG with verified dimensions."""
    if not tres_path.exists():
        raise FileNotFoundError(f"Source tres not found: {tres_path}")

    content: str = tres_path.read_text(encoding="utf-8")
    match: re.Match[str] | None = re.search(r"PackedByteArray\((.*?)\)", content, re.DOTALL)
    if not match:
        raise ValueError(f"No PackedByteArray found in {tres_path}")

    raw_tokens: list[str] = [x.strip() for x in match.group(1).split(",") if x.strip()]
    raw_bytes: bytes = bytes(int(t) for t in raw_tokens)
    expected_size: int = width * height * 4
    if len(raw_bytes) != expected_size:
        raise ValueError(f"Byte length mismatch in {tres_path}: got {len(raw_bytes)}, expected {expected_size}")

    output_png_path.parent.mkdir(parents=True, exist_ok=True)
    image: Image.Image = Image.frombytes("RGBA", (width, height), raw_bytes)
    image.save(output_png_path, "PNG")

# <META - ROLE : Migrates physical images, shaders, and recipes to assetdb and cleans obsolete files | L55-82>
def migrate_physical_files() -> None:
    """Copy physical files to assetdb and remove duplicates and obsolete tres files."""
    root: Path = Path.cwd()

    _copy_files(root / "features/entity/weapon/asset/img", root / "assetdb/entity/weapon", (".png",))
    _copy_files(root / "features/entity/weapon/asset", root / "assetdb/entity/weapon", (".gdshader",))
    _copy_files(root / "features/entity/weapon/asset/recipe", root / "assetdb/entity/weapon", (".json",))

    _copy_files(root / "features/entity/enemy/asset/img", root / "assetdb/entity/enemy", (".png",))
    _copy_files(root / "features/entity/player/asset/img", root / "assetdb/entity/player", (".png",))
    _copy_files(root / "features/entity/player/asset", root / "assetdb/entity/player", (".gdshader",))

    _copy_files(root / "features/world/object/asset/img", root / "assetdb/world/object", (".png",))
    _copy_files(root / "features/ui/stage/asset/img", root / "assetdb/ui/hud", (".png",), exclude=DUPLICATE_WEAPON_NAMES)

    _copy_files(root / "core/asset/audio", root / "assetdb/audio", (".ogg", ".wav", ".mp3"))
    _copy_files(root / "core/asset/img", root / "assetdb/ui/theme", (".png",))

    ui_img_dir: Path = root / "features/ui/stage/asset/img"
    for dup_name in DUPLICATE_WEAPON_NAMES:
        dup_path: Path = ui_img_dir / dup_name
        if dup_path.exists():
            dup_path.unlink()

    for tres_rel in ("features/world/stage/asset/room_tileset.tres", "features/world/lobby/lobby_tileset.tres"):
        tres_p: Path = root / tres_rel
        if tres_p.exists():
            tres_p.unlink()

# <META - ROLE : Main entry point orchestrating pure PNG extraction and asset migration | L85-98>
def main() -> None:
    """Execute asset migration and pure PNG extraction."""
    root: Path = Path.cwd()
    room_tres: Path = root / "features/world/stage/asset/room_tileset.tres"
    room_png: Path = root / "assetdb/world/stage/room_tileset.png"
    if room_tres.exists():
        extract_png_from_tres(room_tres, room_png, 256, 64)

    lobby_tres: Path = root / "features/world/lobby/lobby_tileset.tres"
    lobby_png: Path = root / "assetdb/world/lobby/lobby_tileset.png"
    if lobby_tres.exists():
        extract_png_from_tres(lobby_tres, lobby_png, 192, 64)

    migrate_physical_files()

if __name__ == "__main__":
    main()
