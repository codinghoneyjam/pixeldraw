# Copy-forward from <game>/dev/tools/assets/v2/parity/version.py. Byte-identical body.
# Three docstring references inside point at game-repo files
# (tools/assets/tests/test_bar_profiles.py, docs/40_work/reports/asset_parity/)
# and are left as written on purpose: editing them would fork the copy from
# the original. The game keeps its own copy because its tests import it.
# <META - FILE SUMMARY - Pillow version gate ensuring pixel comparisons are reproducible>
"""Pillow version gate for asset parity. A bit-exact or ≥99% comparison across different Pillow builds is meaningless."""

from __future__ import annotations

PINNED_PILLOW_VERSION: str = "12.1.0"

_INSTALL_HINT: str = "tools/requirements.txt"

# <META - ROLE : Return the version string of the running Pillow build | L11-28>
def resolve_running_pillow() -> str:
    """Return the version string of the Pillow build actually running.

    Never falls back to a placeholder: an unresolvable version would make
    downstream pixel comparisons silently incomparable.

    Raises:
        RuntimeError: If ``PIL`` cannot be imported.
    """
    try:
        import PIL
    except ImportError as exc:
        raise RuntimeError(
            "Pillow is not importable, so asset parity cannot be evaluated. "
            f"Install the pinned build from {_INSTALL_HINT}: "
            f"Pillow=={PINNED_PILLOW_VERSION}"
        ) from exc
    return str(PIL.__version__)

# <META - ROLE : Fail loudly unless running Pillow matches expected version exactly | L31-50>
def assert_pillow_matches(expected: str) -> None:
    """Fail loudly unless the running Pillow equals ``expected`` exactly.

    Comparison is full string equality, so a partial or prefix form such as
    ``"12.1"`` never satisfies ``"12.1.0"``. There is no warn mode, no
    override flag, and no auto-install: a bypass would let incomparable
    pixels pass as comparable.

    Raises:
        RuntimeError: If the running version differs from ``expected``.
    """
    running: str = resolve_running_pillow()
    if running == expected:
        return None
    raise RuntimeError(
        "Pillow version mismatch: any pixel comparison against this build is "
        f"not reproducible. running={running!r} expected={expected!r}. "
        f"Install the pinned build from {_INSTALL_HINT}: "
        f"Pillow=={PINNED_PILLOW_VERSION}"
    )
