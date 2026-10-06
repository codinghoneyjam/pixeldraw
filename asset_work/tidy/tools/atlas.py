# <META - FILE SUMMARY - Canonical atlas builder that packs images into a grid PNG>
"""Canonical atlas builder shared by asset generators."""
from pathlib import Path
from typing import Iterable, Sequence
from PIL import Image

class AtlasBuilder:
    # <META - ROLE : Store slot size and column count for atlas layout | L9-11>
    def __init__(self, slot_size: tuple[int, int] = (128, 128), columns: int | None = None):
        self.slot_size = slot_size
        self.columns = columns

    # <META - ROLE : Pack a sequence of images into a grid atlas PNG with optional preview | L14-28>
    def build(self, images: Sequence[Image.Image], output: Path, *, preview: Path | None = None) -> Path:
        if not images:
            raise ValueError("Atlas requires at least one image")
        width, height = self.slot_size
        columns = self.columns or len(images)
        rows = (len(images) + columns - 1) // columns
        atlas = Image.new("RGBA", (width * columns, height * rows), (0, 0, 0, 0))
        for index, image in enumerate(images):
            atlas.paste(image.convert("RGBA").resize(self.slot_size), ((index % columns) * width, (index // columns) * height), image.convert("RGBA").resize(self.slot_size))
        output.parent.mkdir(parents=True, exist_ok=True)
        atlas.save(output, "PNG")
        if preview:
            preview.parent.mkdir(parents=True, exist_ok=True)
            atlas.save(preview, "PNG")
        return output
