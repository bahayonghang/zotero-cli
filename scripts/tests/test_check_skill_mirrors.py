from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.check_skill_mirrors import compare_trees


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


if __name__ == "__main__":
    unittest.main()
