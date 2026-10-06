# <META - FILE SUMMARY - Project-root path resolver for the asset pipeline (src, dst, res)>
"""Project-root path SSOT for the asset pipeline, split across three axes.

`PROJECT_ROOT` is **consumed** from `tools.data.config` -- this module never
defines a root of its own (AGENTS.md forbids a sixth definition).

Three resolvers, and the split between them is the whole point:

| axis | helper   | redirected by `ASSET_OUT_ROOT`? |
|------|----------|----------------------------------|
| input  | `src()`  | **no** -- recipes, specs, fonts    |
| output | `dst()`  | **yes**                          |
| res:// | `res()`  | **no** -- manifest strings         |

Why inputs and outputs must not share a redirect: generators read their recipes
from `src()`. If `ASSET_OUT_ROOT` redirected inputs too, every redirected run
would fail to find its shapes/specs/profiles. Why `res()` must not be
redirected either: those strings are consumed by Godot at *runtime*, not by the
bake tool, so a temp path in a manifest would simply be wrong.

Precedence, highest first:

1. an explicit `--output` on the command line -- used verbatim, never re-rooted
2. `ASSET_OUT_ROOT`
3. `PROJECT_ROOT`

`ASSET_OUT_ROOT` is read **once, at import time**. That is deliberate and
load-bearing: a test (or a caller) that sets the variable *after* importing
this module gets no redirect. Use `re_resolve()` to re-read the environment
deliberately, and read its return value rather than trusting a constant --
`OUT_ROOT` is a mutable module global for exactly that reason.
"""
from __future__ import annotations

import os
import sys
from collections.abc import Mapping
from pathlib import Path

from dev.tools.data.config import PROJECT_ROOT

__all__ = [
    "PROJECT_ROOT",
    "OUT_ROOT",
    "OUT_ROOT_ENV",
    "src",
    "dst",
    "res",
    "re_resolve",
    "SHAPES_ENEMY",
    "SHAPES_HUD_ICONS",
    "SHAPES_WEAPON",
    "SPECS_ENEMY",
    "SPECS_PLAYER",
    "PROFILES_KEYCAP",
    "SHAPE_PORTAL_KEYCAP",
    "IMAGES_PORTAL",
    "PROFILE_PANTOGRAPH",
    "FONT_MONOGRAM",
    "MANIFEST_HUD_SURFACES",
    "TEX_ENEMY",
    "IMAGES_UI",
    "ATLAS_UI",
    "ATLAS_UI_WRITE",
    "MANIFEST_UI",
    "IMAGES_PANTOGRAPH",
    "CORE_KEYCAP",
]

OUT_ROOT_ENV: str = "ASSET_OUT_ROOT"

SHAPES_ENEMY: str = "assetdb/entity/enemy"
SPECS_ENEMY: str = "assetdb/entity/enemy/enemy.json"
SHAPES_WEAPON: str = "assetdb/entity/weapon"
SHAPES_HUD_ICONS: str = "assetdb/ui/data"
SPECS_PLAYER: str = "assetdb/entity/player/player.json"
PROFILES_KEYCAP: str = "assetdb/world/object/keycap_visual_profiles.json"
SHAPE_PORTAL_KEYCAP: str = "assetdb/world/object/portal_keycap_master.json"
IMAGES_PORTAL: str = "assetdb/world/img/portal"
PROFILE_PANTOGRAPH: str = "features/ui/data/pantograph_keycap_profile.json"
FONT_MONOGRAM: str = "core/asset/font/monogram.ttf"
MANIFEST_HUD_SURFACES: str = "assetdb/ui/data/manifests/hud_composite_surfaces.json"

TEX_ENEMY: str = "assetdb/entity/enemy"
IMAGES_UI: str = "assetdb/ui/hud"

ATLAS_UI: str = "assetdb/ui/hud"

ATLAS_UI_WRITE: str = "assetdb/ui/hud"

MANIFEST_UI: str = "assetdb/ui/data"
IMAGES_PANTOGRAPH: str = "assetdb/ui/hud"
CORE_KEYCAP: str = "core/asset/img"

OUT_ROOT: Path | None = None
_OVERRIDE_REPORTED: bool = False

# <META - ROLE : Re-read ASSET_OUT_ROOT env var and update output root | L95-120>
def re_resolve(env: Mapping[str, str] | None = None) -> Path | None:
    """Re-read `ASSET_OUT_ROOT` and return the active output root, or None.

    Import-time behaviour is the call `re_resolve()` with no argument, made
    once by this module. Callers that need a different environment (tests, in
    particular) pass one explicitly instead of mutating `os.environ` behind
    this module's back -- setting the variable after import without calling
    this is the one way to get a silently un-redirected run.
    """
    global OUT_ROOT, _OVERRIDE_REPORTED
    source: Mapping[str, str] = os.environ if env is None else env
    raw: str | None = source.get(OUT_ROOT_ENV)
    if raw is None or not raw.strip():
        OUT_ROOT = None
    else:
        candidate: Path = Path(raw)
        if not candidate.is_absolute():
            raise ValueError(
                f"{OUT_ROOT_ENV} must be an absolute path; got relative value {raw!r}. "
                f"A relative value would be resolved against the current working directory, "
                f"which is the exact bug this module exists to remove. "
                f"Pass an absolute path, or unset {OUT_ROOT_ENV}."
            )
        OUT_ROOT = candidate
    _OVERRIDE_REPORTED = False
    return OUT_ROOT

# <META - ROLE : Resolve an input path under PROJECT_ROOT | L123-125>
def src(rel: str) -> Path:
    """Resolve an input path under `PROJECT_ROOT`. Never redirected."""
    return PROJECT_ROOT / rel

# <META - ROLE : Resolve an output path under ASSET_OUT_ROOT or PROJECT_ROOT | L128-133>
def dst(rel: str) -> Path:
    """Resolve an output path under `ASSET_OUT_ROOT`, else under `PROJECT_ROOT`."""
    if OUT_ROOT is not None:
        _report_override()
        return OUT_ROOT / rel
    return PROJECT_ROOT / rel

# <META - ROLE : Build a res:// manifest string from a relative path | L136-138>
def res(rel: str) -> str:
    """Build a `res://` manifest string. Never redirected -- Godot reads it."""
    return "res://" + rel.replace("\\", "/")

# <META - ROLE : Print a one-time warning about active output redirect | L141-151>
def _report_override() -> None:
    """Announce an active output redirect on stderr, once per process."""
    global _OVERRIDE_REPORTED
    if _OVERRIDE_REPORTED or OUT_ROOT is None:
        return
    _OVERRIDE_REPORTED = True
    print(
        f"[paths] WARNING: {OUT_ROOT_ENV} override ACTIVE -> outputs redirected to {OUT_ROOT}\n"
        f"[paths] WARNING: this is a redirected run; `res://` manifest strings are unaffected.",
        file=sys.stderr,
    )

re_resolve()
