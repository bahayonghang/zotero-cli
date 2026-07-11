#!/usr/bin/env python3
"""Verify generated zot skill mirrors match the canonical source tree."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


IGNORED_NAMES = frozenset({".DS_Store", "__pycache__"})


def collect_tree(root: Path) -> dict[str, bytes]:
    if not root.is_dir():
        raise FileNotFoundError(f"skill tree does not exist: {root}")

    files: dict[str, bytes] = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if any(part in IGNORED_NAMES for part in relative.parts):
            continue
        if path.is_file():
            files[relative.as_posix()] = path.read_bytes()
    return files


def compare_trees(canonical: Path, mirror: Path) -> list[str]:
    source = collect_tree(canonical)
    generated = collect_tree(mirror)
    issues: list[str] = []

    for relative in sorted(source.keys() - generated.keys()):
        issues.append(f"missing: {relative}")
    for relative in sorted(generated.keys() - source.keys()):
        issues.append(f"extra: {relative}")
    for relative in sorted(source.keys() & generated.keys()):
        if source[relative] == generated[relative]:
            continue
        source_hash = hashlib.sha256(source[relative]).hexdigest()[:12]
        mirror_hash = hashlib.sha256(generated[relative]).hexdigest()[:12]
        issues.append(
            f"content: {relative} (canonical={source_hash}, mirror={mirror_hash})"
        )

    return issues


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--canonical", type=Path, default=Path("skills/zot"))
    parser.add_argument("--mirror", type=Path, action="append")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    mirrors = args.mirror or [Path(".agents/skills/zot"), Path(".claude/skills/zot")]
    failed = False

    try:
        for mirror in mirrors:
            issues = compare_trees(args.canonical, mirror)
            if not issues:
                print(f"skill mirror matches canonical: {mirror}")
                continue
            failed = True
            print(f"skill mirror drift: {mirror}")
            for issue in issues:
                print(f"  - {issue}")
    except FileNotFoundError as error:
        print(error)
        return 1

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
