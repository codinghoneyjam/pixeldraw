from __future__ import annotations

import argparse
from collections import defaultdict
from pathlib import Path


TEXT_EXTENSIONS = {
    ".bat",
    ".cjs",
    ".cmd",
    ".css",
    ".html",
    ".ini",
    ".jsx",
    ".js",
    ".json",
    ".md",
    ".mjs",
    ".ps1",
    ".py",
    ".sh",
    ".sql",
    ".toml",
    ".ts",
    ".tsx",
    ".txt",
    ".xml",
    ".yaml",
    ".yml",
}
EXCLUDED_DIRS = {".git", ".pytest_cache", ".venv", "__pycache__", "node_modules", "venv"}


def measure_files(root: Path, excluded_file: Path) -> tuple[list[tuple[Path, int | None, int]], list[Path]]:
    measured = []
    unreadable = []
    excluded_file = excluded_file.resolve()

    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        if any(part in EXCLUDED_DIRS for part in path.relative_to(root).parts):
            continue
        if path.resolve() == excluded_file:
            continue
        try:
            size_bytes = path.stat().st_size
        except OSError:
            unreadable.append(path)
            continue

        lines = None
        if path.suffix.lower() in TEXT_EXTENSIONS:
            try:
                lines = len(path.read_text(encoding="utf-8-sig").splitlines())
            except (OSError, UnicodeError):
                unreadable.append(path)
        measured.append((path.relative_to(root), lines, size_bytes))

    return measured, unreadable


def human_size(size_bytes: int) -> str:
    size = float(size_bytes)
    for unit in ("B", "KiB", "MiB", "GiB", "TiB"):
        if size < 1024 or unit == "TiB":
            return f"{size:.2f} {unit}"
        size /= 1024
    return f"{size_bytes} B"


def format_table(headers: list[str], rows: list[list[str]], right_aligned: set[int]) -> list[str]:
    widths = [max([len(headers[i]), *(len(row[i]) for row in rows)]) for i in range(len(headers))]

    def render(row: list[str]) -> str:
        cells = [cell.rjust(widths[i]) if i in right_aligned else cell.ljust(widths[i])
                 for i, cell in enumerate(row)]
        return "| " + " | ".join(cells) + " |"

    return [render(headers), "| " + " | ".join("-" * width for width in widths) + " |"] + [
        render(row) for row in rows
    ]


def aggregate_folders(root: Path, measured: list[tuple[Path, int | None, int]]) -> dict[str, list[int]]:
    totals: dict[str, list[int]] = defaultdict(lambda: [0, 0, 0])
    totals["."]
    for path in root.rglob("*"):
        if path.is_dir() and not any(part in EXCLUDED_DIRS for part in path.relative_to(root).parts):
            totals[path.relative_to(root).as_posix()]

    for relative_path, lines, size_bytes in measured:
        folder = relative_path.parent
        while True:
            key = folder.as_posix() if folder.parts else "."
            totals[key][0] += 1
            totals[key][1] += lines or 0
            totals[key][2] += size_bytes
            if not folder.parts:
                break
            folder = folder.parent
    return totals


def build_report(
    root: Path,
    measured: list[tuple[Path, int | None, int]],
    folder_totals: dict[str, list[int]],
    unreadable: list[Path],
) -> str:
    total_files, total_lines, total_bytes = folder_totals["."]
    text_files = sum(lines is not None for _, lines, _ in measured)

    def folder_row(folder: str, values: list[int]) -> list[str]:
        file_count, line_count, size_bytes = values
        return [folder, f"{file_count:,}", f"{line_count:,}", human_size(size_bytes), f"{size_bytes:,}"]

    top_domains = [
        folder_row(folder, values)
        for folder, values in sorted(folder_totals.items())
        if len(Path(folder).parts) == 1
    ]
    domain_sums = [
        sum(values[index] for folder, values in folder_totals.items() if len(Path(folder).parts) == 1)
        for index in range(3)
    ]
    root_files = [folder_totals["."][index] - domain_sums[index] for index in range(3)]
    if any(root_files):
        top_domains.insert(0, folder_row("(root files)", root_files))
    all_folders = [
        folder_row(folder, values)
        for folder, values in sorted(folder_totals.items(), key=lambda item: (item[0].count("/"), item[0]))
    ]
    all_files = [
        [
            relative_path.as_posix(),
            f"{lines:,}" if lines is not None else "-",
            human_size(size_bytes),
            f"{size_bytes:,}",
        ]
        for relative_path, lines, size_bytes in sorted(measured, key=lambda item: item[0].as_posix())
    ]

    output = [
        "PixelDraw domain line-count and size report",
        "============================================",
        f"Root: {root}",
        f"Files measured (all formats): {total_files:,}",
        f"Text files with line counts: {text_files:,}",
        f"Total text lines: {total_lines:,}",
        f"Total size: {human_size(total_bytes)} ({total_bytes:,} bytes)",
        "Line counts: UTF-8 files with listed extensions; physical lines via splitlines().",
        "Sizes: actual file bytes for all formats, including images and other binary assets.",
        "Line-count extensions: " + ", ".join(sorted(TEXT_EXTENSIONS)),
        "Excluded directories: " + ", ".join(sorted(EXCLUDED_DIRS)),
        "",
        "TOP-LEVEL DOMAINS (recursive totals)",
        "Parent and child folder rows overlap; do not sum both sections together.",
    ]
    output.extend(format_table(["Domain", "Files", "Lines", "Size", "Bytes"], top_domains, {1, 2, 3, 4}))
    output.extend(["", "ALL FOLDERS (recursive totals)"])
    output.extend(format_table(["Domain / folder", "Files", "Lines", "Size", "Bytes"], all_folders, {1, 2, 3, 4}))
    output.extend(["", "ALL FILES ('-' line count means binary or unlisted extension)"])
    output.extend(format_table(["File", "Lines", "Size", "Bytes"], all_files, {1, 2, 3}))
    output.extend(["", "The output report itself is excluded from its own measurement."])

    if unreadable:
        output.extend(["", "UNREADABLE FILES", "----------------"])
        output.extend(path.relative_to(root).as_posix() for path in unreadable)

    return "\n".join(output) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description="Report text lines and on-disk size by folder and file.")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path, default=None, help="Report path (default: <root>/line_counts.txt)")
    args = parser.parse_args()

    root = args.root.resolve()
    output = (args.output or root / "line_counts.txt").resolve()
    measured, unreadable = measure_files(root, output)
    folder_totals = aggregate_folders(root, measured)
    report = build_report(root, measured, folder_totals, unreadable)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(report, encoding="utf-8")
    print(f"Wrote {output}")
    file_count, line_count, size_bytes = folder_totals["."]
    print(f"Measured {file_count:,} files, {line_count:,} text lines, {human_size(size_bytes)} ({size_bytes:,} bytes).")


if __name__ == "__main__":
    main()