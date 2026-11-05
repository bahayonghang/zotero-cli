from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
TRELLIS_SCRIPTS = ROOT / ".trellis" / "scripts"
with patch.object(sys, "path", [str(TRELLIS_SCRIPTS), *sys.path]):
    from common.trellis_config import read_trellis_config
    from common.workflow_phase import (
        _platform_matches,
        filter_platform,
        resolve_effective_platform,
    )


class PlatformMatchingTests(unittest.TestCase):
    def test_aliases_ignore_case_and_separators_in_both_directions(self) -> None:
        groups = {
            "Claude Code": ("claude", "CLAUDE", "claude-code", "CLAUDE_CODE"),
            "Kimi Code": ("kimi", "KIMI", "kimi-code", "KIMI_CODE"),
            "Oh My Pi": ("omp", "OMP", "oh-my-pi", "OH_MY_PI"),
        }
        for canonical, variants in groups.items():
            for variant in variants:
                with self.subTest(canonical=canonical, variant=variant):
                    self.assertTrue(_platform_matches(variant, [canonical]))
                    self.assertTrue(_platform_matches(canonical, [variant]))

    def test_other_platforms_keep_existing_matching_rules(self) -> None:
        for platform, marker in (
            ("grok", "Grok"),
            ("cursor", "Cursor"),
            ("custom-tool", "CUSTOM_TOOL"),
            ("codex-inline", "Codex Inline"),
            ("codex-sub-agent", "CODEX_SUB_AGENT"),
        ):
            with self.subTest(platform=platform, marker=marker):
                self.assertTrue(_platform_matches(platform, [marker]))
        for platform, marker in (
            ("omp", "Pi"),
            ("claude", "Claude Desktop"),
            ("kimi", "Grok"),
            ("unknown", "Claude Code"),
            ("codex-inline", "codex-sub-agent"),
        ):
            with self.subTest(platform=platform, marker=marker):
                self.assertFalse(_platform_matches(platform, [marker]))

    def test_filter_keeps_only_matching_body_and_common_text(self) -> None:
        content = (
            "Shared start\n"
            "[Claude Code]\nClaude body\n[/Claude Code]\n"
            "[Grok]\nGrok body\n[/Grok]\n"
            "[Pi]\nPi body\n[/Pi]\n"
            "[Oh My Pi]\nOMP body\n[/Oh My Pi]\n"
            "Shared end\n"
        )
        for platform, body in (
            ("claude", "Claude body\n"),
            ("grok", "Grok body\n"),
            ("omp", "OMP body\n"),
            ("pi", "Pi body\n"),
            ("unknown", ""),
        ):
            with self.subTest(platform=platform):
                self.assertEqual(
                    filter_platform(content, platform),
                    f"Shared start\n{body}Shared end\n",
                )


class CodexDispatchTests(unittest.TestCase):
    def test_default_explicit_legacy_and_invalid_modes(self) -> None:
        cases = (
            ({}, "codex-sub-agent"),
            ({"codex": {}}, "codex-sub-agent"),
            ({"codex": None}, "codex-sub-agent"),
            ({"codex": {"dispatch_mode": "auto"}}, "codex-sub-agent"),
            ({"codex": {"dispatch_mode": "sub-agent"}}, "codex-sub-agent"),
            ({"codex": {"dispatch_mode": " AUTO "}}, "codex-sub-agent"),
            ({"codex": {"dispatch_mode": "inline"}}, "codex-inline"),
            ({"codex": {"dispatch_mode": " INLINE "}}, "codex-inline"),
            ({"codex": {"dispatch_mode": "invalid"}}, "codex-inline"),
            ({"codex": {"dispatch_mode": None}}, "codex-inline"),
            ({"codex": {"dispatch_mode": ""}}, "codex-inline"),
            ({"codex": "invalid"}, "codex-inline"),
        )
        for config, expected in cases:
            with self.subTest(config=config):
                self.assertEqual(resolve_effective_platform("codex", config), expected)

    def test_dispatch_config_does_not_change_other_platforms(self) -> None:
        for platform in ("claude", "grok", "kimi", "omp", "cursor", "unknown"):
            with self.subTest(platform=platform):
                self.assertEqual(
                    resolve_effective_platform(platform, {"codex": {"dispatch_mode": "inline"}}),
                    platform,
                )


class WorkflowContextCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp_dir.cleanup)
        self.repo = Path(self.temp_dir.name)
        trellis = self.repo / ".trellis"
        trellis.mkdir()
        # Exercise the real workflow and CLI with isolated task/config state.
        (trellis / "workflow.md").write_bytes((ROOT / ".trellis" / "workflow.md").read_bytes())

    def render(self, platform: str, step: str, *, cwd: Path | None = None) -> str:
        result = subprocess.run(
            [
                sys.executable,
                "-B",
                str(TRELLIS_SCRIPTS / "get_context.py"),
                "--mode",
                "phase",
                "--step",
                step,
                "--platform",
                platform,
            ],
            cwd=cwd or self.repo,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=30,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout

    def assert_step_body(self, body: str, step: str, *, inline: bool = False) -> None:
        self.assertIn(f"#### {step} ", body)
        if step == "2.1":
            if inline:
                self.assertIn("trellis-before-dev", body)
                self.assertIn("Read `{TASK_DIR}/prd.md`", body)
                self.assertIn("Implement the code per reviewed artifacts", body)
                self.assertNotIn("Spawn the implement sub-agent:", body)
            else:
                self.assertIn("Spawn the implement sub-agent:", body)
                self.assertIn("**Agent type**: `trellis-implement`", body)
                self.assertIn("Active task: <task path>", body)
                self.assertIn("lint and type-check", body)
        elif inline:
            self.assertIn("Load the `trellis-check` skill and verify", body)
            self.assertIn("Spec compliance", body)
            self.assertIn("lint / type-check / tests", body)
            self.assertNotIn("Spawn the check sub-agent:", body)
        else:
            self.assertIn("Spawn the check sub-agent:", body)
            self.assertIn("**Agent type**: `trellis-check`", body)
            self.assertIn("Review code changes against specs", body)
            self.assertIn("Auto-fix issues it finds", body)

    def test_five_platforms_render_implementation_and_check_bodies(self) -> None:
        for platform in ("claude", "codex", "grok", "kimi", "omp"):
            for step in ("2.1", "2.2"):
                with self.subTest(platform=platform, step=step):
                    self.assert_step_body(self.render(platform, step), step)

    def test_short_ids_match_canonical_names_with_meaningful_bodies(self) -> None:
        for platform, canonical in (
            ("claude", "Claude Code"),
            ("kimi", "Kimi Code"),
            ("omp", "Oh My Pi"),
        ):
            for step in ("2.1", "2.2"):
                with self.subTest(platform=platform, step=step):
                    body = self.render(platform, step)
                    self.assert_step_body(body, step)
                    self.assertEqual(body, self.render(canonical, step))

    def test_codex_config_modes_reach_correct_cli_bodies(self) -> None:
        cases = (
            ("", False),
            ("codex:\n", False),
            ("codex:\n  dispatch_mode: auto\n", False),
            ("codex:\n  dispatch_mode: sub-agent\n", False),
            ("codex:\n  dispatch_mode: inline\n", True),
            ("codex:\n  dispatch_mode: invalid\n", True),
            ("codex: invalid\n", True),
        )
        for config, inline in cases:
            (self.repo / ".trellis" / "config.yaml").write_text(config, encoding="utf-8")
            for step in ("2.1", "2.2"):
                with self.subTest(config=config, step=step):
                    self.assert_step_body(self.render("codex", step), step, inline=inline)

    def test_unrelated_inline_platform_keeps_its_cli_body(self) -> None:
        for step in ("2.1", "2.2"):
            with self.subTest(step=step):
                self.assert_step_body(self.render("antigravity", step), step, inline=True)

    def test_current_repository_codex_config_reaches_expected_cli_body(self) -> None:
        effective = resolve_effective_platform("codex", read_trellis_config(ROOT))
        for step in ("2.1", "2.2"):
            with self.subTest(step=step, effective=effective):
                self.assert_step_body(
                    self.render("codex", step, cwd=ROOT),
                    step,
                    inline=effective == "codex-inline",
                )


if __name__ == "__main__":
    unittest.main()
