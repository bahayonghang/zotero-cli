from __future__ import annotations

import io
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

from scripts.check_skill_mirrors import compare_trees, main


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


if __name__ == "__main__":
    unittest.main()
