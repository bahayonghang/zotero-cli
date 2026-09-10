#!/usr/bin/env python3
"""Verify generated skill mirrors match every published canonical skill tree."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


IGNORED_NAMES = frozenset({".DS_Store", "__pycache__"})
SKILLS_ROOT = Path("skills")
MANAGED_MIRROR_ROOTS = (Path(".agents/skills"), Path(".claude/skills"))


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


def discover_published_skills(skills_root: Path) -> list[Path]:
    if not skills_root.is_dir():
        raise FileNotFoundError(f"skill tree does not exist: {skills_root}")

    published: list[Path] = []
    for child in sorted(skills_root.iterdir()):
        if child.is_dir() and (child / "SKILL.md").is_file():
            published.append(child)
    return published


def managed_mirrors(skill_name: str) -> list[Path]:
    return [root / skill_name for root in MANAGED_MIRROR_ROOTS]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--canonical",
        type=Path,
        default=None,
        help="canonical skill tree (default: every skills/*/SKILL.md)",
    )
    parser.add_argument(
        "--mirror",
        type=Path,
        action="append",
        help="generated mirror tree (default: .agents/skills/<name> and .claude/skills/<name>)",
    )
    parser.add_argument(
        "--skip-if-all-missing",
        action="store_true",
        help="skip comparison when no generated mirror is installed",
    )
    return parser.parse_args()


def check_jobs(args: argparse.Namespace) -> list[tuple[Path, list[Path]]]:
    if args.canonical is not None or args.mirror:
        canonical = args.canonical or Path("skills/zot")
        mirrors = args.mirror or managed_mirrors(canonical.name)
        return [(canonical, mirrors)]

    return [
        (skill, managed_mirrors(skill.name))
        for skill in discover_published_skills(SKILLS_ROOT)
    ]


def report_mirrors(canonical: Path, mirrors: list[Path]) -> bool:
    failed = False
    for mirror in mirrors:
        issues = compare_trees(canonical, mirror)
        if not issues:
            print(f"skill mirror matches canonical: {mirror}")
            continue
        failed = True
        print(f"skill mirror drift: {mirror}")
        for issue in issues:
            print(f"  - {issue}")
    return failed


def main() -> int:
    args = parse_args()
    failed = False

    try:
        jobs = check_jobs(args)
        all_mirrors = [mirror for _canonical, mirrors in jobs for mirror in mirrors]
        if args.skip_if_all_missing and all(not mirror.exists() for mirror in all_mirrors):
            for canonical, _mirrors in jobs:
                collect_tree(canonical)
            print("skill mirrors are not installed; skipping local mirror comparison")
            return 0

        for canonical, mirrors in jobs:
            if report_mirrors(canonical, mirrors):
                failed = True
    except FileNotFoundError as error:
        print(error)
        return 1

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
