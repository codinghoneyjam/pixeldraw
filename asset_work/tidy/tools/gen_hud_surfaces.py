#!/usr/bin/env python3
# <META - FILE SUMMARY - Build Stage HUD composite surface PNGs from declarative manifest>
"""Build Stage HUD composite surface PNGs from the declarative manifest.

Usage:
  python -m dev.tools.assets.generators.gen_hud_surfaces
  python -m dev.tools.assets.generators.gen_hud_surfaces --validate
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from PIL import Image

from dev.tools.assets.core import canvas, paths

DEFAULT_MANIFEST: Path = paths.src(paths.MANIFEST_HUD_SURFACES)
RECIPE_GROUPS: tuple[str, ...] = ("surfaces", "atlases")
SUPPORTED_LAYER_TYPES: frozenset[str] = frozenset({"rounded_rect", "rect", "line", "polygon"})

# <META - ROLE : Load and parse a composite-surface manifest JSON file | L26-34>
def _load_manifest(manifest_path: Path) -> dict[str, Any]:
    """Load one composite-surface manifest as a JSON object."""
    try:
        payload: Any = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"manifest 읽기 실패: {error}") from error
    if not isinstance(payload, dict):
        raise ValueError("manifest 최상위 값은 객체여야 합니다.")
    return payload

# <META - ROLE : Resolve a manifest output path within project boundaries | L37-49>
def _project_output(raw_output: Any) -> Path:
    """Resolve a manifest output while preventing paths outside the project."""
    if not isinstance(raw_output, str) or not raw_output.strip():
        raise ValueError("output은 비어 있지 않은 프로젝트 상대 경로여야 합니다.")
    relative_output: Path = Path(raw_output)
    if relative_output.is_absolute():
        raise ValueError(f"절대 output 경로는 허용되지 않습니다: {raw_output}")
    target: Path = (paths.PROJECT_ROOT / relative_output).resolve()
    try:
        target.relative_to(paths.PROJECT_ROOT)
    except ValueError as error:
        raise ValueError(f"프로젝트 밖 output 경로는 허용되지 않습니다: {raw_output}") from error
    return target

# <META - ROLE : Validate one surface or atlas recipe structure and output path | L52-83>
def _validate_recipe(group: str, recipe: Any, outputs: set[Path]) -> list[str]:
    """Validate one declared surface or atlas recipe before rendering."""
    if not isinstance(recipe, dict):
        return [f"{group}: 항목은 객체여야 합니다."]
    recipe_id: str = str(recipe.get("id", "<missing>"))
    canvas_spec: Any = recipe.get("canvas")
    layers: Any = recipe.get("layers")
    errors: list[str] = []
    if not recipe.get("id"):
        errors.append(f"{group}: id 누락")
    if not isinstance(canvas_spec, dict):
        errors.append(f"{recipe_id}: canvas 객체 누락")
    else:
        width: Any = canvas_spec.get("width")
        height: Any = canvas_spec.get("height")
        if not isinstance(width, int) or not isinstance(height, int) or width <= 0 or height <= 0:
            errors.append(f"{recipe_id}: canvas width/height는 양의 정수여야 합니다.")
    if not isinstance(layers, list) or not layers:
        errors.append(f"{recipe_id}: layers 배열이 비어 있거나 누락되었습니다.")
    else:
        for index, layer in enumerate(layers):
            layer_type: Any = layer.get("type") if isinstance(layer, dict) else None
            if layer_type not in SUPPORTED_LAYER_TYPES:
                errors.append(f"{recipe_id}: layers[{index}]의 type이 지원되지 않습니다: {layer_type!r}")
    try:
        target: Path = _project_output(recipe.get("output"))
        if target in outputs:
            errors.append(f"{recipe_id}: output 경로가 중복됩니다: {target.relative_to(paths.PROJECT_ROOT)}")
        outputs.add(target)
    except ValueError as error:
        errors.append(f"{recipe_id}: {error}")
    return errors

# <META - ROLE : Validate entire manifest and collect all structural errors | L86-98>
def validate_manifest(manifest_path: Path) -> tuple[dict[str, Any], list[str]]:
    """Return a parsed manifest and every structural validation error."""
    manifest: dict[str, Any] = _load_manifest(manifest_path)
    outputs: set[Path] = set()
    errors: list[str] = []
    for group in RECIPE_GROUPS:
        recipes: Any = manifest.get(group)
        if not isinstance(recipes, list) or not recipes:
            errors.append(f"{group} 배열이 비어 있거나 누락되었습니다.")
            continue
        for recipe in recipes:
            errors.extend(_validate_recipe(group, recipe, outputs))
    return manifest, errors

# <META - ROLE : Render all validated manifest recipes and verify RGBA outputs | L101-116>
def build(manifest_path: Path) -> list[Path]:
    """Render every validated manifest recipe and verify its RGBA output."""
    manifest, errors = validate_manifest(manifest_path)
    if errors:
        raise ValueError("\n".join(errors))
    written: list[Path] = []
    for group in RECIPE_GROUPS:
        for recipe in manifest[group]:
            target: Path = _project_output(recipe["output"])
            canvas.run_recipe(recipe, output_override=str(target))
            with Image.open(target) as image:
                expected: dict[str, Any] = recipe["canvas"]
                if image.mode != "RGBA" or image.size != (expected["width"], expected["height"]):
                    raise ValueError(f"{target.relative_to(paths.PROJECT_ROOT)}: PNG mode/size 검증 실패")
            written.append(target)
    return written

# <META - ROLE : CLI entrypoint to validate or build HUD composite surfaces | L119-143>
def main() -> int:
    """Validate or build the current Stage HUD composite surface manifest."""
    parser = argparse.ArgumentParser(description="Stage HUD composite surface generator")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--validate", action="store_true", help="manifest만 검증하고 PNG를 쓰지 않습니다.")
    args = parser.parse_args()
    manifest_path: Path = args.manifest.resolve()
    if not manifest_path.exists():
        print(f"[ERROR] manifest 없음: {manifest_path}")
        return 1
    try:
        _manifest, errors = validate_manifest(manifest_path)
        if errors:
            for error in errors:
                print(f"[ERROR] {error}")
            return 1
        if args.validate:
            print(f"[VALIDATE] {manifest_path.relative_to(paths.PROJECT_ROOT)} — OK")
            return 0
        written: list[Path] = build(manifest_path)
    except ValueError as error:
        print(f"[ERROR] {error}")
        return 1
    print(f"[BUILD] {len(written)}개 composite surface 출력 생성")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
