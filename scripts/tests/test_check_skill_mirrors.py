from __future__ import annotations

import io
import os
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

from scripts.check_skill_mirrors import compare_trees, main

DEFAULT_ARGV = ["check_skill_mirrors.py", "--skip-if-all-missing"]
ZOT_BODY = "zot-canonical\n"
BRAINSTORM_BODY = "brainstorm-canonical\n"
THIRD_BODY = "third-canonical\n"


def _write_skill(root: Path, name: str, content: str) -> None:
    directory = root / name
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "SKILL.md").write_text(content, encoding="utf-8")


def _publish_skills(
    repo: Path,
    bodies: dict[str, str],
    *,
    mirrored_names: set[str] | None = None,
    mirror_roots: tuple[str, ...] = (".agents/skills", ".claude/skills"),
) -> None:
    if mirrored_names is None:
        mirrored_names = set(bodies)
    for name, content in bodies.items():
        _write_skill(repo / "skills", name, content)
        if name not in mirrored_names:
            continue
        for mirror_root in mirror_roots:
            _write_skill(repo / mirror_root, name, content)


def _file_snapshot(root: Path) -> dict[str, bytes]:
    snapshot: dict[str, bytes] = {}
    for path in sorted(root.rglob("*")):
        if path.is_file():
            snapshot[path.relative_to(root).as_posix()] = path.read_bytes()
    return snapshot


def _run_main(argv: list[str], cwd: Path) -> tuple[int, str]:
    previous = os.getcwd()
    buffer = io.StringIO()
    try:
        os.chdir(cwd)
        with patch("sys.argv", argv), redirect_stdout(buffer):
            code = main()
    finally:
        # Windows cannot remove TemporaryDirectory while it is cwd (WinError 32).
        os.chdir(previous)
    return code, buffer.getvalue()


def _match_line(mirror: str) -> str:
    return f"skill mirror matches canonical: {Path(mirror)}"


def _drift_line(mirror: str) -> str:
    return f"skill mirror drift: {Path(mirror)}"


class SkillMirrorComparisonTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        root = Path(self.temp_dir.name)
        self.canonical = root / "canonical"
        self.mirror = root / "mirror"
        (self.canonical / "evals").mkdir(parents=True)
        (self.mirror / "evals").mkdir(parents=True)
        (self.canonical / "SKILL.md").write_text("canonical\n", encoding="utf-8")
        (self.mirror / "SKILL.md").write_text("canonical\n", encoding="utf-8")
        (self.canonical / "evals" / "evals.json").write_text("{}\n", encoding="utf-8")
        (self.mirror / "evals" / "evals.json").write_text("{}\n", encoding="utf-8")

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_identical_trees_match(self) -> None:
        self.assertEqual(compare_trees(self.canonical, self.mirror), [])

    def test_content_drift_fails(self) -> None:
        (self.mirror / "SKILL.md").write_text("drift\n", encoding="utf-8")
        self.assertTrue(
            any(issue.startswith("content: SKILL.md") for issue in compare_trees(self.canonical, self.mirror))
        )

    def test_missing_and_extra_files_fail(self) -> None:
        (self.mirror / "evals" / "evals.json").unlink()
        (self.mirror / "unexpected.txt").write_text("extra\n", encoding="utf-8")
        issues = compare_trees(self.canonical, self.mirror)
        self.assertIn("missing: evals/evals.json", issues)
        self.assertIn("extra: unexpected.txt", issues)

    def test_all_uninstalled_mirrors_can_be_skipped(self) -> None:
        root = Path(self.temp_dir.name)
        mirrors = [root / "agents-mirror", root / "claude-mirror"]
        argv = [
            "check_skill_mirrors.py",
            "--canonical",
            str(self.canonical),
            "--mirror",
            str(mirrors[0]),
            "--mirror",
            str(mirrors[1]),
            "--skip-if-all-missing",
        ]

        output = io.StringIO()
        with patch("sys.argv", argv), redirect_stdout(output):
            self.assertEqual(main(), 0)
        self.assertIn("skill mirrors are not installed", output.getvalue())

    def test_partial_mirror_install_still_fails(self) -> None:
        missing_mirror = Path(self.temp_dir.name) / "missing-mirror"
        argv = [
            "check_skill_mirrors.py",
            "--canonical",
            str(self.canonical),
            "--mirror",
            str(self.mirror),
            "--mirror",
            str(missing_mirror),
            "--skip-if-all-missing",
        ]

        output = io.StringIO()
        with patch("sys.argv", argv), redirect_stdout(output):
            self.assertEqual(main(), 1)
        self.assertIn(f"skill tree does not exist: {missing_mirror}", output.getvalue())


class SkillMirrorDefaultEntryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.repo = Path(self.temp_dir.name)
        self.original_cwd = os.getcwd()

    def tearDown(self) -> None:
        os.chdir(self.original_cwd)
        self.temp_dir.cleanup()

    def test_default_entry_fails_when_second_skill_drifts(self) -> None:
        _publish_skills(
            self.repo,
            {"zot": ZOT_BODY, "zot-brainstorm": BRAINSTORM_BODY},
        )
        (
            self.repo / ".agents" / "skills" / "zot-brainstorm" / "SKILL.md"
        ).write_text("drift\n", encoding="utf-8")

        code, output = _run_main(DEFAULT_ARGV, self.repo)

        self.assertEqual(code, 1)
        self.assertIn(_match_line(".agents/skills/zot"), output)
        self.assertIn(_match_line(".claude/skills/zot"), output)
        self.assertIn(_drift_line(".agents/skills/zot-brainstorm"), output)
        self.assertIn("content: SKILL.md", output)

    def test_default_entry_passes_when_all_skills_match(self) -> None:
        _publish_skills(
            self.repo,
            {"zot": ZOT_BODY, "zot-brainstorm": BRAINSTORM_BODY},
        )

        code, output = _run_main(DEFAULT_ARGV, self.repo)

        self.assertEqual(code, 0)
        self.assertIn(_match_line(".agents/skills/zot"), output)
        self.assertIn(_match_line(".claude/skills/zot"), output)
        self.assertIn(_match_line(".agents/skills/zot-brainstorm"), output)
        self.assertIn(_match_line(".claude/skills/zot-brainstorm"), output)

    def test_default_entry_includes_new_canonical_skill(self) -> None:
        _publish_skills(
            self.repo,
            {
                "zot": ZOT_BODY,
                "zot-brainstorm": BRAINSTORM_BODY,
                "third-skill": THIRD_BODY,
            },
        )
        (self.repo / ".claude" / "skills" / "third-skill" / "SKILL.md").write_text(
            "third-drift\n", encoding="utf-8"
        )

        code, output = _run_main(DEFAULT_ARGV, self.repo)

        self.assertEqual(code, 1)
        self.assertIn(_drift_line(".claude/skills/third-skill"), output)
        self.assertIn(_match_line(".agents/skills/third-skill"), output)

    def test_default_entry_skips_when_all_managed_mirrors_missing(self) -> None:
        _publish_skills(
            self.repo,
            {"zot": ZOT_BODY, "zot-brainstorm": BRAINSTORM_BODY},
            mirrored_names=set(),
        )

        code, output = _run_main(DEFAULT_ARGV, self.repo)

        self.assertEqual(code, 0)
        self.assertIn("skill mirrors are not installed", output)

    def test_default_entry_fails_when_one_skill_mirrors_are_missing(self) -> None:
        _publish_skills(
            self.repo,
            {"zot": ZOT_BODY, "zot-brainstorm": BRAINSTORM_BODY},
            mirrored_names={"zot"},
        )

        code, output = _run_main(DEFAULT_ARGV, self.repo)

        self.assertEqual(code, 1)
        self.assertIn("skill tree does not exist:", output)
        self.assertIn("zot-brainstorm", output.replace("\\", "/"))

    def test_default_entry_fails_when_one_mirror_root_is_missing(self) -> None:
        _publish_skills(
            self.repo,
            {"zot": ZOT_BODY, "zot-brainstorm": BRAINSTORM_BODY},
            mirror_roots=(".agents/skills",),
        )

        code, output = _run_main(DEFAULT_ARGV, self.repo)

        self.assertEqual(code, 1)
        self.assertIn("skill tree does not exist:", output)
        self.assertIn(".claude", output.replace("\\", "/"))

    def test_default_entry_ignores_unrelated_mirror_directories(self) -> None:
        _publish_skills(
            self.repo,
            {"zot": ZOT_BODY, "zot-brainstorm": BRAINSTORM_BODY},
        )
        _write_skill(
            self.repo / ".agents" / "skills",
            "unrelated-helper",
            "not a published skill\n",
        )

        code, output = _run_main(DEFAULT_ARGV, self.repo)

        self.assertEqual(code, 0)
        self.assertNotIn("unrelated-helper", output)
        self.assertIn(_match_line(".agents/skills/zot-brainstorm"), output)

    def test_default_entry_does_not_write_files(self) -> None:
        _publish_skills(
            self.repo,
            {"zot": ZOT_BODY, "zot-brainstorm": BRAINSTORM_BODY},
        )
        before = _file_snapshot(self.repo)

        code, _output = _run_main(DEFAULT_ARGV, self.repo)

        self.assertEqual(code, 0)
        self.assertEqual(_file_snapshot(self.repo), before)


if __name__ == "__main__":
    unittest.main()
