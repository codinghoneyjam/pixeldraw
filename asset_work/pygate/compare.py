# Copy-forward from <game>/dev/tools/assets/v2/parity/compare.py. Byte-identical body.
# Three docstring references inside point at game-repo files
# (tools/assets/tests/test_bar_profiles.py, docs/40_work/reports/asset_parity/)
# and are left as written on purpose: editing them would fork the copy from
# the original. The game keeps its own copy because its tests import it.
# <META - FILE SUMMARY - Compare decoded RGBA pixels for asset parity, measuring divergence metrics>
"""Decoded-RGBA comparison for asset parity (plan S1, sections 4.1-4.2).

PNG **bytes** are never compared. PNG chunking, filter selection and compression
level all vary between Pillow/zlib builds, so the byte stream diverges even when
the rendered image is identical -- that is the recorded reason
`tools/assets/tests/test_bar_profiles.py` hashes the decoded buffer instead of
the saved file (its module docstring, lines 1-14). This module keeps that rule.

`decoded_digest` is a copy-forward of that module's `pixel_digest` expression,
not a reimplementation of it: `pixel_digest` takes a `Path` and opens the file,
while `compare_images` receives already-decoded `Image` objects, so the one-line
formula is reused verbatim and `tests/test_parity.py` pins the two together. The
original is never edited.

Alpha-0 RGB is deliberately NOT special-cased (plan section 11, Q12 is still
open). The full 4-byte RGBA tuple is compared, so this module never decides a
question that is not its own to decide.

`label_components` and `DivergenceComponent` were added by AS2c, and they exist
because of a **granularity** correction rather than a new measurement. AS2b read
compactness and density off the whole-image `bbox`, which is still here and still
correct -- but the real Q8 divergence is six separate compact elements on a
16-slot sheet, so the whole-image box is 13.98% of the canvas against a 3.125%
ceiling and the spatial test refused a real regression. The thresholds were
never wrong. What a consumer now needs instead is the same measurement taken per
**connected component**, so this module labels them. The classification
consequences, and why amplitude is per component too, are in
`classify.py`'s module docstring; the derivation is in
`docs/40_work/reports/asset_parity/AS2c_component_derivation.md`.
"""

from __future__ import annotations

import hashlib
import re
from bisect import bisect_right
from dataclasses import dataclass

from PIL import Image, ImageChops

Bbox = tuple[int, int, int, int]

_Run = tuple[int, int, int]

_ALPHA_BAND: int = 3
_OPAQUE: int = 255
_BAND_COUNT: int = 4

_NONZERO_RUN: re.Pattern[bytes] = re.compile(rb"[^\x00]+")

_SUPPORTED_CONNECTIVITY: frozenset[int] = frozenset({4, 8})

@dataclass(frozen=True)
class DivergenceComponent:
    """One connected group of differing pixels, measured on its own.

    Every term is computed at label time over *this component only*, which is
    the point of the type: AS2b read its geometry off the whole-image bbox, and
    that measured a real six-element divergence as 13.98% of the canvas, i.e.
    incoherent. Each element's own bbox makes every term scale to the thing it
    describes.

    * `bbox`: this component's box, `getbbox` convention (right/bottom exclusive).
    * `size`: differing pixels in it.
    * `fraction_of_canvas`: `bbox_area / total_pixels`, 0.0 on an empty canvas.
    * `density`: `size / bbox_area`, 0.0 for a degenerate box.
    * `max_channel_delta`: largest peak **inside it**, not across the image.
    """

    bbox: Bbox
    size: int
    fraction_of_canvas: float
    density: float
    max_channel_delta: int

@dataclass(frozen=True)
class ComparisonResult:
    """One file's verdict, carrying everything the report must print.

    * `identical`: True when no pixel differs in any channel.
    * `total_pixels`: Width x height of the decoded canvas.
    * `differing_pixels`: Pixels whose 4-byte RGBA tuple differs.
    * `match_fraction`: 1.0 - differing_pixels / total_pixels.
    * `max_channel_delta`: Largest absolute per-channel delta, 0 when identical.
    * `one_sided_ratio`: Share of differing pixels that went opaque -> transparent
      or transparent -> opaque, the ELEMENT_DELTA tell.
    * `bbox`: Bounding box across all four channels, or None when identical.
      Whole-image, and unchanged, because `report.py` consumes it.
    * `reference_digest` / `candidate_digest`: sha256 of the decoded RGBA buffers.
    * `components`: Connected groups, biggest first. Defaults to empty so a
      hand-built result that predates component labelling still constructs; every
      field after it has no default, so this default cannot be dropped.
    """

    identical: bool
    total_pixels: int
    differing_pixels: int
    match_fraction: float
    max_channel_delta: int
    one_sided_ratio: float
    bbox: Bbox | None
    reference_digest: str
    candidate_digest: str
    components: tuple[DivergenceComponent, ...] = ()

# <META - ROLE : Compute sha256 of decoded RGBA pixels | L108-114>
def decoded_digest(image: Image.Image) -> str:
    """Return the sha256 of the decoded RGBA pixels, ignoring PNG encoding.

    Copy-forward of `test_bar_profiles.pixel_digest` (`tools/assets/tests/
    test_bar_profiles.py:33`), which is the sanctioned digest for this project.
    """
    return hashlib.sha256(image.convert("RGBA").tobytes()).hexdigest()

# <META - ROLE : Decode both images to RGBA and refuse a size mismatch | L117-134>
def _rgba_pair(reference: Image.Image, candidate: Image.Image) -> tuple[Image.Image, Image.Image]:
    """Decode both images to RGBA and refuse a size mismatch.

    A resized image is a different artefact, not a slightly worse one, so
    scoring it would be meaningless. Raising keeps that loud.

    Raises:
        ValueError: If the two decoded sizes differ.
    """
    left: Image.Image = reference.convert("RGBA")
    right: Image.Image = candidate.convert("RGBA")
    if left.size != right.size:
        raise ValueError(
            "size mismatch: a resized image is a different artefact and cannot be "
            f"scored. reference={left.size[0]}x{left.size[1]} "
            f"candidate={right.size[0]}x{right.size[1]}"
        )
    return left, right

# <META - ROLE : Return size-checked absolute RGBA difference | L137-144>
def rgba_difference(reference: Image.Image, candidate: Image.Image) -> Image.Image:
    """Return the size-checked absolute RGBA difference of the two images.

    The shared primitive behind every other measurement in this module, exposed
    so `report.py` can amplify the same difference instead of recomputing it.
    """
    left, right = _rgba_pair(reference, candidate)
    return ImageChops.difference(left, right)

# <META - ROLE : Collapse a 4-band difference into one per-pixel-max band | L147-157>
def peak_difference(difference: Image.Image) -> Image.Image:
    """Collapse a 4-band difference into one "L" band of per-pixel maxima.

    `value = max(|dR|, |dG|, |dB|, |dA|)` per pixel, so a single histogram or
    extrema call answers "did this pixel change?" and "by how much?" at once.
    """
    bands = difference.split()
    peak: Image.Image = bands[0]
    for index in range(1, _BAND_COUNT):
        peak = ImageChops.lighter(peak, bands[index])
    return peak

# <META - ROLE : Count nonzero pixels in a single-band mask | L160-162>
def _nonzero_count(mask: Image.Image) -> int:
    """Count pixels of a single-band mask whose value is not 0."""
    return mask.size[0] * mask.size[1] - mask.histogram()[0]

# <META - ROLE : Return the largest absolute per-channel difference | L165-172>
def max_channel_delta(reference: Image.Image, candidate: Image.Image) -> int:
    """Return the largest absolute per-channel difference; 0 when identical.

    Raises:
        ValueError: If the two decoded sizes differ.
    """
    extrema = rgba_difference(reference, candidate).getextrema()
    return max(high for _, high in extrema)

# <META - ROLE : Return the bounding box of divergence across all channels | L175-187>
def divergence_bbox(reference: Image.Image, candidate: Image.Image) -> Bbox | None:
    """Return the bounding box of the divergence, or None when identical.

    `getbbox` defaults to `alpha_only=True`, so on an RGBA image it reports the
    box of the *alpha* difference alone and returns None for an image that
    differs only in RGB. The bounding box is a scale-free review signal (plan
    section 4.4), so it has to span every channel: `alpha_only=False` is passed
    explicitly rather than relying on the default.

    Raises:
        ValueError: If the two decoded sizes differ.
    """
    return rgba_difference(reference, candidate).getbbox(alpha_only=False)

# <META - ROLE : Measure share of differing pixels that flipped opaque/transparent | L190-210>
def _one_sided_ratio(left: Image.Image, right: Image.Image, differing: int) -> float:
    """Share of differing pixels that flipped between fully opaque and transparent.

    Anti-aliased edge pixels move between partial alphas, never between 255 and
    0, so a scattered antialiasing sweep stays near 0.0 while a removed graphic
    element sits at 1.0. Every one-sided pixel necessarily differs (255 vs 0 in
    the alpha band), so the count can never exceed `differing`.
    """
    if differing == 0:
        return 0.0
    alpha_left: Image.Image = left.getchannel(_ALPHA_BAND)
    alpha_right: Image.Image = right.getchannel(_ALPHA_BAND)
    opaque_left = alpha_left.point(lambda value: _OPAQUE if value == _OPAQUE else 0)
    clear_right = alpha_right.point(lambda value: _OPAQUE if value == 0 else 0)
    clear_left = alpha_left.point(lambda value: _OPAQUE if value == 0 else 0)
    opaque_right = alpha_right.point(lambda value: _OPAQUE if value == _OPAQUE else 0)
    one_sided = ImageChops.lighter(
        ImageChops.multiply(opaque_left, clear_right),
        ImageChops.multiply(clear_left, opaque_right),
    )
    return _nonzero_count(one_sided) / differing

# <META - ROLE : Find root of a node in union-find with path compression | L213-223>
def _find_root(parent: dict[int, int], node: int) -> int:
    """Return the root of `node` in the union-find forest, compressing the path."""
    root: int = node
    while parent[root] != root:
        root = parent[root]
    cursor: int = node
    while parent[cursor] != root:
        successor: int = parent[cursor]
        parent[cursor] = root
        cursor = successor
    return root

# <META - ROLE : Extract maximal horizontal nonzero runs from a mask buffer | L226-245>
def _row_runs(data: bytes, width: int) -> list[_Run]:
    """Maximal horizontal runs of nonzero bytes, row-major then x-ascending.

    `_NONZERO_RUN` scans the whole W*H buffer once in C, so the Python body runs
    once per *run* rather than once per pixel. That is the whole cost argument:
    a fully divergent 2048x128 sheet is 128 runs here, not 262,144 iterations.
    A run straddling a row boundary is split, because a pixel at x = W-1 and one
    at x = 0 of the next row are not adjacent and the mask has no row separator.
    """
    runs: list[_Run] = []
    for match in _NONZERO_RUN.finditer(data):
        start, stop = match.span()
        first_row, _ = divmod(start, width)
        last_row, _ = divmod(stop - 1, width)
        for row in range(first_row, last_row + 1):
            origin = row * width
            begin = start - origin if row == first_row else 0
            end = stop - origin if row == last_row else width
            runs.append((row, begin, end))
    return runs

# <META - ROLE : Map each occupied row to its first run index | L248-254>
def _row_spans(runs: list[_Run]) -> dict[int, int]:
    """Map each occupied row to the index of its first run inside `runs`."""
    firsts: dict[int, int] = {}
    for index, run in enumerate(runs):
        if run[0] not in firsts:
            firsts[run[0]] = index
    return firsts

# <META - ROLE : Group nonzero runs into connected components via union-find | L257-296>
def _label(data: bytes, width: int, connectivity: int) -> list[list[_Run]]:
    """Group every nonzero run into its connected component.

    Two-phase union-find over *runs*, not pixels. Runs are extracted in C, then
    swept row by row; a run links to the runs of the row above whose column span
    it overlaps, widened by one column under 8-connectivity. Both `start` and
    `end` ascend across a row, so `bisect_right` on the previous row's `end`
    values gives the first candidate that can reach, and an early `break` on
    `start` drops the rest: O(R log R) for R runs, amortised O(1) linking.
    """
    runs: list[_Run] = _row_runs(data, width)
    firsts: dict[int, int] = _row_spans(runs)
    reach: int = 1 if connectivity == 8 else 0
    parent: dict[int, int] = {}
    previous: list[int] = []
    ends: list[int] = []
    previous_row: int = -2
    for index, run in enumerate(runs):
        row, x_start, x_end = run
        if row != previous_row:
            previous_row = row
            below = firsts.get(row - 1)
            previous = list(range(below, firsts[row])) if below is not None else []
            ends = [runs[slot][2] for slot in previous]
        found: set[int] = set()
        for slot in range(bisect_right(ends, x_start - reach), len(previous)):
            if runs[previous[slot]][1] >= x_end + reach:
                break
            found.add(_find_root(parent, previous[slot]))
        if not found:
            parent[index] = index
            continue
        lowest: int = min(found)
        for root in found:
            parent[root] = lowest
        parent[index] = lowest
    groups: dict[int, list[_Run]] = {}
    for index, run in enumerate(runs):
        groups.setdefault(_find_root(parent, index), []).append(run)
    return list(groups.values())

# <META - ROLE : Measure one labelled run group into a DivergenceComponent | L299-319>
def _summarise(
    runs: list[_Run], peak_bytes: bytes, width: int, total_pixels: int
) -> DivergenceComponent:
    """Measure one labelled group of runs into a `DivergenceComponent`."""
    rows: list[int] = [run[0] for run in runs]
    left: int = min(run[1] for run in runs)
    right: int = max(run[2] for run in runs)
    bbox: Bbox = (left, min(rows), right, max(rows) + 1)
    area: int = (bbox[2] - bbox[0]) * (bbox[3] - bbox[1])
    size: int = sum(run[2] - run[1] for run in runs)
    peak: int = 0
    for row, run_left, run_right in runs:
        origin = row * width
        peak = max(peak, max(peak_bytes[origin + run_left : origin + run_right]))
    return DivergenceComponent(
        bbox=bbox,
        size=size,
        fraction_of_canvas=area / total_pixels if total_pixels else 0.0,
        density=size / area if area > 0 else 0.0,
        max_channel_delta=peak,
    )

# <META - ROLE : Label connected pixel groups in a peak mask by run-based union-find | L322-367>
def label_components(
    peak: Image.Image, *, connectivity: int = 4
) -> tuple[DivergenceComponent, ...]:
    """Label the connected groups of differing pixels in the "L" peak mask.

    Takes the **existing** `peak` mask -- the same buffer `differing_pixels` is
    counted from -- and never re-walks the source images, so this is not a
    second full pass over the pixels of the pair.

    **Complexity.** Run extraction is one C pass over the W*H buffer
    (`re.finditer`), and the sweep is O(R log R) for R runs with amortised O(1)
    union-find linking. Crucially the Python work scales with **runs**, not
    pixels: the real 608-pixel Q8 divergence is a few dozen runs and a fully
    divergent 2048x128 sheet is 128. A per-pixel labeller was measured at
    332 ms on that worst case, against 1.0 ms here and 5.6 ms for the whole
    AS2b comparison; `AS2c_component_derivation.md` section 7 has the table.

    `connectivity` is keyword-only, defaults to 4 (cross/plus), and accepts 8
    (king) for the sensitivity check. 4- and 8-connected labelling of the real
    fixture are measured to agree exactly, so the default is not a guess and the
    parameter keeps that claim testable against this code rather than against a
    test-local re-implementation.

    Args:
        peak: The single-band per-pixel-maximum mask from `peak_difference`.
        connectivity: 4 for cross/plus, 8 for the full king move.

    Returns:
        Components sorted by size descending, then by bbox, so report output is
        byte-stable across runs. Empty when the mask is empty.

    Raises:
        ValueError: If `connectivity` is neither 4 nor 8.
    """
    if connectivity not in _SUPPORTED_CONNECTIVITY:
        raise ValueError(
            f"connectivity must be 4 (cross) or 8 (king), not {connectivity!r}"
        )
    width = peak.size[0]
    peak_bytes: bytes = peak.tobytes()
    total_pixels: int = peak.size[0] * peak.size[1]
    labelled: list[DivergenceComponent] = [
        _summarise(runs, peak_bytes, width, total_pixels)
        for runs in _label(peak_bytes, width, connectivity)
    ]
    return tuple(sorted(labelled, key=lambda item: (-item.size, item.bbox)))

# <META - ROLE : Compare two images on decoded RGBA and return a ComparisonResult | L370-394>
def compare_images(reference: Image.Image, candidate: Image.Image) -> ComparisonResult:
    """Compare two images on their decoded RGBA buffers, never their PNG bytes.

    Raises:
        ValueError: If either argument is None, or the sizes differ.
    """
    if reference is None or candidate is None:
        raise ValueError("compare_images requires two decoded images, not None")
    left, right = _rgba_pair(reference, candidate)
    total_pixels: int = left.size[0] * left.size[1]
    difference = rgba_difference(left, right)
    peak = peak_difference(difference)
    differing_pixels: int = _nonzero_count(peak)
    return ComparisonResult(
        identical=differing_pixels == 0,
        total_pixels=total_pixels,
        differing_pixels=differing_pixels,
        match_fraction=1.0 - differing_pixels / total_pixels if total_pixels else 1.0,
        max_channel_delta=int(peak.getextrema()[1]),
        one_sided_ratio=_one_sided_ratio(left, right, differing_pixels),
        bbox=difference.getbbox(alpha_only=False),
        reference_digest=decoded_digest(left),
        candidate_digest=decoded_digest(right),
        components=label_components(peak),
    )
