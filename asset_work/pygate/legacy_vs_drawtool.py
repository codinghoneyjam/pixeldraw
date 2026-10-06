# <META - FILE SUMMARY - Python parity gate: render 8 legacy assets via the transpiler and assert decoded-RGBA parity>
"""Python equivalent of tools/recipe_parity_check.mjs (plan T-4b).

An independent implementation of the same gate the JS one runs: if the two
disagree, the fault is in the gate rather than in the tool. It covers 8 of the
JS gate's 24 cases -- the JS gate is a superset, so a pass here is necessary
but not sufficient.

``version`` and ``compare`` are copy-forwarded from the game package
(``dev/tools/assets/v2/parity/``), which the game keeps because its own tests
import them. They are copies rather than reimplementations on purpose: the
number this gate prints is only comparable if it comes out of the same
comparator.

The pinned Pillow is enforced FIRST via ``version.assert_pillow_matches`` before
any image is touched, so a pixel comparison is never made under an unpinned
build. Each of the 8 gate-able assets is rendered by the committed transpiler
(``tools/recipe/transpile.mjs``) through a node subprocess, then compared on
decoded RGBA -- never PNG bytes -- via ``compare.compare_images``.

Classification mirrors ``classify.py``'s amplitude floor: a differing pixel
whose max channel delta is >= 128 is an ELEMENT_DELTA, otherwise RENDER_NOISE.
The divergence bbox is ``Image.getbbox(alpha_only=False)`` so it spans every
channel, exactly as ``compare.divergence_bbox`` measures it.

Usage:
  python asset_work/pygate/legacy_vs_drawtool.py
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

import version
from compare import compare_images

# <META - ROLE : Resolve one target/ asset path | L36-L45>
TOOL_ROOT: Path = Path(__file__).resolve().parents[2]   # pixeldraw/
TARGET_ROOT: Path = TOOL_ROOT / "asset_work" / "target"  # recipes + legacy PNGs
DRAWTOOL_ROOT: Path = TOOL_ROOT


def target_path(rel: str) -> Path:
    """Resolve one asset path against target/ (recipes and legacy PNGs).

    This tool used to live at <game>/draw_tool_v2/, so the cases spelled asset
    paths relative to the game repo and rendered the game's own copies.
    Standalone, every asset it reads lives under pixeldraw/asset_work/target/, which
    carries the same relative paths, so the old strings still resolve once the
    root is repointed.
    """
    return TARGET_ROOT / rel.removeprefix("draw_tool_v2/")


MATCH_FRACTION_THRESHOLD: float = 0.99
ELEMENT_DELTA_CHANNEL_THRESHOLD: int = 128

CASES: tuple[dict[str, str], ...] = (
    {"id": "sword_albedo", "recipe": "assetdb/entity/weapon/weapon_sword.json", "layer": "albedo", "legacy": "assetdb/entity/weapon/sword_albedo.png"},
    {"id": "sword_mask", "recipe": "assetdb/entity/weapon/weapon_sword.json", "layer": "mask", "legacy": "assetdb/entity/weapon/sword_mask.png"},
    {"id": "bow_albedo", "recipe": "assetdb/entity/weapon/weapon_bow.json", "layer": "albedo", "legacy": "assetdb/entity/weapon/bow_albedo.png"},
    {"id": "bow_mask", "recipe": "assetdb/entity/weapon/weapon_bow.json", "layer": "mask", "legacy": "assetdb/entity/weapon/bow_mask.png"},
    {"id": "spear_albedo", "recipe": "assetdb/entity/weapon/weapon_spear.json", "layer": "albedo", "legacy": "assetdb/entity/weapon/spear_albedo.png"},
    {"id": "spear_mask", "recipe": "assetdb/entity/weapon/weapon_spear.json", "layer": "mask", "legacy": "assetdb/entity/weapon/spear_mask.png"},
    {"id": "portal_keycap_unpressed", "recipe": "assetdb/world/object/portal_keycap_master.json", "layer": "keycap_unpressed", "legacy": "assetdb/world/img/portal/portal_keycap_unpressed.png"},
    {"id": "portal_keycap_pressed", "recipe": "assetdb/world/object/portal_keycap_master.json", "layer": "keycap_pressed", "legacy": "assetdb/world/img/portal/portal_keycap_pressed.png"},
)

# <META - ROLE : Render one recipe/layer to a PNG via the transpiler node subprocess | L49-69>
def render_via_transpiler(recipe_rel: str, layer: str, out_png: Path) -> None:
    """Render one recipe layer to ``out_png`` by shelling out to the transpiler.

    The transpiler resolves ``--recipe``/``--out`` against its cwd, so absolute
    paths are passed and the repo root is used as the working directory. The
    transpiler's own progress logs are captured and only surfaced on failure so
    the gate's stdout stays a clean report.
    """
    transpiler: Path = DRAWTOOL_ROOT / "tools" / "recipe" / "transpile.mjs"
    recipe_path: Path = target_path(recipe_rel)
    completed = subprocess.run(
        ["node", str(transpiler), "--recipe", str(recipe_path), "--layer", layer, "--out", str(out_png)],
        cwd=str(TOOL_ROOT),
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"transpiler failed for {recipe_rel} (layer {layer}), "
            f"exit {completed.returncode}:\n{completed.stderr.strip()}"
        )

# <META - ROLE : Classify one comparison via the amplitude floor (delta >= 128) | L72-80>
def classify(result) -> str:
    """Return ELEMENT_DELTA or RENDER_NOISE from the comparison's amplitude.

    Mirrors ``classify.py``'s ``ELEMENT_DELTA_CHANNEL_THRESHOLD`` floor: the
    whole-image max channel delta is the cheap early reject, and any component
    severe on its own terms is an element change. A divergence whose every
    channel moved by less than 128 is render noise.
    """
    return "ELEMENT_DELTA" if result.max_channel_delta >= ELEMENT_DELTA_CHANNEL_THRESHOLD else "RENDER_NOISE"

# <META - ROLE : Compare one case's legacy PNG against a fresh transpiler render | L83-103>
def check_case(case_info: dict[str, str], tmp_dir: Path) -> dict[str, object]:
    """Render, decode and compare one case; return its report row."""
    case_id: str = case_info["id"]
    out_png: Path = tmp_dir / f"{case_id}.png"
    render_via_transpiler(case_info["recipe"], case_info["layer"], out_png)

    legacy_path: Path = target_path(case_info["legacy"])
    with Image.open(legacy_path) as legacy_img, Image.open(out_png) as candidate_img:
        result = compare_images(legacy_img, candidate_img)

    return {
        "id": case_id,
        "classification": classify(result),
        "match_fraction": result.match_fraction,
        "total": result.total_pixels,
        "matches": result.total_pixels - result.differing_pixels,
        "differing": result.differing_pixels,
        "max_delta": result.max_channel_delta,
        "bbox": result.bbox,
        "passed": result.match_fraction >= MATCH_FRACTION_THRESHOLD,
    }

# <META - ROLE : Print one asset row and its failure detail (bbox + digests) | L106-121>
def print_row(row: dict[str, object]) -> None:
    """Print one asset row; on failure add the divergence bbox and digests."""
    case_id = str(row["id"])
    status = "PASS" if bool(row["passed"]) else "FAIL"
    match_fraction = float(row["match_fraction"])
    matches = int(row["matches"])
    total = int(row["total"])
    classification = str(row["classification"])
    max_delta = int(row["max_delta"])
    print(
        f"[{status}] {case_id:26} match={match_fraction:.6f}  "
        f"{matches}/{total}  class={classification}  max_delta={max_delta}"
    )
    if status == "FAIL":
        bbox = row["bbox"]
        print(f"       divergence_bbox={bbox}")

# <META - ROLE : Run all 8 cases, print the report and summary, set exit code | L124-143>
def main() -> int:
    """Enforce the pinned Pillow, run all 8 cases, print the report, set exit."""
    version.assert_pillow_matches(version.PINNED_PILLOW_VERSION)
    print(f"legacy_vs_drawtool: {len(CASES)} assets, threshold {MATCH_FRACTION_THRESHOLD}")
    print(f"pillow={version.PINNED_PILLOW_VERSION} (pinned)")

    passed: int = 0
    with tempfile.TemporaryDirectory(prefix="legacy-vs-drawtool-") as tmp_name:
        tmp_dir = Path(tmp_name)
        for case_info in CASES:
            row = check_case(case_info, tmp_dir)
            print_row(row)
            if bool(row["passed"]):
                passed += 1

    total = len(CASES)
    print("--")
    print(f"RESULT: {passed}/{total} passed (threshold {MATCH_FRACTION_THRESHOLD})")
    print("parity gate: OK" if passed == total else "parity gate: FAILED")
    return 0 if passed == total else 1

if __name__ == "__main__":
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    raise SystemExit(main())
