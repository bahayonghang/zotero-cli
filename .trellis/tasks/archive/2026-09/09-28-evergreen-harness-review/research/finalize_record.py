"""Record local acceptance without committing or archiving the task tree."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
TASKS = [
    "09-28-evergreen-harness-review", "09-28-security-lock-refresh",
    "09-28-harness-platform-routing", "09-28-harness-contract-refresh",
    "09-28-evergreen-security-cadence",
]
PRODUCTS = [
    ".github/workflows/ci.yml", ".trellis/config.yaml",
    ".trellis/scripts/common/workflow_phase.py",
    ".trellis/spec/zot-cli/backend/quality-guidelines.md",
    "AGENTS.md", "Cargo.lock", "README.md", "README.zh-CN.md",
    "docs/agents/harnesses.md", "scripts/tests/test_harness_platform_routing.py",
]
def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT).decode("utf-8").strip()

now = datetime.now(timezone.utc).isoformat()
for name in TASKS:
    path = ROOT / ".trellis/tasks" / name / "task.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["meta"]["approval_status"] == "approved"
    data["status"] = "review"
    data["completedAt"] = None
    data["meta"].update(local_acceptance="passed", local_verified_at_utc=now,
        execution_report=".trellis/tasks/09-28-evergreen-harness-review/execution-report.md",
        commit_status="not_requested", archive_status="not_requested",
        hosted_validation="UNVERIFIED")
    data["notes"] = "Approved implementation and local checks are complete. Changes remain uncommitted for delivery; task is not archived."
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

parent = json.loads((ROOT / ".trellis/tasks" / TASKS[0] / "task.json").read_text(encoding="utf-8"))
assert set(parent["children"]) == set(TASKS[1:])
for name in TASKS[1:]:
    child = json.loads((ROOT / ".trellis/tasks" / name / "task.json").read_text(encoding="utf-8"))
    assert child["parent"] == TASKS[0]

results = []
for name in TASKS:
    command = [sys.executable, ".trellis/scripts/task.py", "validate", f".trellis/tasks/{name}"]
    checked = subprocess.run(command, cwd=ROOT, capture_output=True)
    log = f"final-validate-{name}.log"
    (OUT / log).write_bytes(checked.stdout + checked.stderr)
    results.append({"task": name, "command": command, "exit_code": checked.returncode, "log": log})
validation = {"checked_utc": now, "parent_child_links": "PASS", "tasks": results}
(OUT / "final-task-validation.json").write_text(json.dumps(validation, indent=2) + "\n", encoding="utf-8")
assert all(result["exit_code"] == 0 for result in results)

tracked = git("diff", "--name-only", "HEAD").splitlines()
assert set(tracked) == set(PRODUCTS[:-1])
assert not git("diff", "--cached", "--name-only"), "Unexpected staged changes"
untracked = subprocess.check_output(["git", "ls-files", "--others", "--exclude-standard", "-z"], cwd=ROOT).decode().split("\0")
prefixes = tuple(f".trellis/tasks/{name}/" for name in TASKS)
assert all(not path or path == PRODUCTS[-1] or path.startswith(prefixes) for path in untracked)
blank_check = subprocess.run(["git", "diff", "--check"], cwd=ROOT, capture_output=True)
(OUT / "final-diff-check.log").write_bytes(blank_check.stdout + blank_check.stderr)
assert blank_check.returncode == 0
new_test = (ROOT / PRODUCTS[-1]).read_text(encoding="utf-8")
assert all(line == line.rstrip() for line in new_test.splitlines())
assert new_test.endswith("\n")
protected = ["Cargo.toml", "CLAUDE.md", "skills", ".github/workflows/deploy-docs.yml", ".trellis/.template-hashes.json"]
assert not git("diff", "--name-only", "HEAD", "--", *protected)
hashes = {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in PRODUCTS}
assert hashes["Cargo.lock"] == "857974bce8e060ed25212b68463e2bd1e0c42be3dce908517d001428d93e706f"
artifacts = {}
for name in TASKS:
    for path in sorted((ROOT / ".trellis/tasks" / name).rglob("*")):
        if path.is_file() and "__pycache__" not in path.parts and path.name != "final-provenance.json":
            artifacts[path.relative_to(ROOT).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
record = {"checked_utc": now, "baseline_commit": git("rev-parse", "HEAD"),
          "new_commit": None, "branch": git("branch", "--show-current"),
          "scope_check": "PASS", "diff_check": "PASS", "protected_files": "UNCHANGED",
          "product_sha256": hashes, "task_artifact_sha256": artifacts,
          "task_status": "review", "local_acceptance": "passed"}
(OUT / "final-provenance.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"result": "PASS", "tasks_validated": len(results), "product_files": len(hashes),
                  "task_artifacts": len(artifacts), "task_status": "review", "new_commit": None}))
