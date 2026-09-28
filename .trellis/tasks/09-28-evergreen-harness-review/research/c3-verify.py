from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
FILES = [
    "AGENTS.md",
    "docs/agents/harnesses.md",
    ".trellis/config.yaml",
    "README.md",
    "README.zh-CN.md",
]
env = dict(os.environ, PYTHONIOENCODING="utf-8")
results = []


def run(name: str, args: list[str]) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        args, cwd=ROOT, env=env, capture_output=True, text=True,
        encoding="utf-8", timeout=60,
    )
    (OUT / f"c3-{name}.log").write_text(
        result.stdout + result.stderr, encoding="utf-8"
    )
    results.append({"name": name, "args": args, "exit_code": result.returncode})
    return result


sys.path.insert(0, str(ROOT / ".trellis" / "scripts"))
from common.trellis_config import read_trellis_config
from common.workflow_phase import resolve_effective_platform

effective = resolve_effective_platform("codex", read_trellis_config(ROOT)
)
assert effective == "codex-inline", effective
for step, required in (
    ("2.1", ["trellis-before-dev", "Read `{TASK_DIR}/prd.md`",
             "Implement the code per reviewed artifacts"]),
    ("2.2", ["Load the `trellis-check` skill and verify",
             "Spec compliance", "lint / type-check / tests"]),
):
    result = run(
        f"codex-{step}",
        [sys.executable, "-B", ".trellis/scripts/get_context.py",
         "--mode", "phase", "--step", step, "--platform", "codex"],
    )
    assert result.returncode == 0, result.stderr
    for expected in required:
        assert expected in result.stdout, (step, expected)
    assert "Spawn the implement sub-agent:" not in result.stdout
    assert "Spawn the check sub-agent:" not in result.stdout
    results[-1]["semantic_assertions"] = "PASS"

assert run("scoped-diff-check", ["git", "diff", "--check", "--", *FILES]).returncode == 0
assert run("protected-files", [
    "git", "diff", "--exit-code", "HEAD", "--", "CLAUDE.md",
    "skills", ".trellis/.template-hashes.json",
]).returncode == 0
assert (ROOT / "CLAUDE.md").read_text(encoding="utf-8").strip() == "@AGENTS.md"
doc = (ROOT / "docs/agents/harnesses.md").read_text(encoding="utf-8")
for version in ("2.1.283", "0.157.1", "1.0.42", "2.0.0", "18.3.5"):
    assert version in doc, version
assert ".trellis/tasks/" not in doc
assert "2026-09-07" not in doc
assert "have no Trellis auto platform marker" not in doc
assert "have no Trellis auto platform marker" not in (ROOT / "AGENTS.md").read_text(encoding="utf-8")
head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
payload = {
    "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
    "head_before_commit": head,
    "scope": "C3 working-tree verification; HEAD excludes these uncommitted edits",
    "effective_platform": effective,
    "checks": results,
    "source_hashes_sha256": {
        name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in FILES
    },
    "omitted": ["just ci and docs build delegated to parent final gate",
                "fresh-start/hook/model/sandbox runtime trials not authorized"],
}
(OUT / "c3-verification.json").write_text(
    json.dumps(payload, ensure_ascii=True, indent=2) + "\n", encoding="utf-8"
)
print(json.dumps(payload, ensure_ascii=True, indent=2))
