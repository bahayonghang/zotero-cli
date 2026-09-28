"""Check the approved CI event delta against the pre-change Git baseline."""
from datetime import datetime, timezone
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import yaml

ROOT = Path(__file__).resolve().parents[4]
BASE = "ba4e10e987e7afe65dccb8450445d41bc9e08d79"
FILE = ".github/workflows/ci.yml"
raw = (ROOT / FILE).read_bytes()
before = yaml.load(subprocess.check_output(["git", "show", f"{BASE}:{FILE}"], cwd=ROOT), Loader=yaml.BaseLoader)
after = yaml.load(raw, Loader=yaml.BaseLoader)
assert set(after) == set(before)
assert after["on"]["schedule"] == [{"cron": "17 8 * * 1"}]
events = copy.deepcopy(after["on"])
events.pop("schedule")
assert events == before["on"]
assert set(after["jobs"]) == set(before["jobs"])
assert after["name"] == before["name"]
assert after["env"] == before["env"]
for name, current in after["jobs"].items():
    job = copy.deepcopy(current)
    assert not job.get("needs"), f"Unexpected job dependency: {name}"
    assert "continue-on-error" not in job
    assert all("continue-on-error" not in step for step in job["steps"])
    if name == "security":
        assert job.pop("permissions") == {"contents": "read"}
        assert "if" not in job
    else:
        assert job.pop("if") == "${{ github.event_name != 'schedule' }}"
    assert job == before["jobs"][name], f"Unapproved job delta: {name}"
matrix = {}
for event in ("push", "pull_request", "workflow_dispatch", "schedule"):
    jobs = [name for name in after["jobs"] if name == "security" or event != "schedule"]
    expanded_count = sum(len(after["jobs"][name].get("strategy", {}).get("matrix", {}).get("os", [None])) for name in jobs)
    matrix[event] = {"jobs": jobs, "expanded_jobs": expanded_count}
result = {"baseline_commit": BASE, "workflow_sha256": hashlib.sha256(raw).hexdigest(),
          "checked_utc": datetime.now(timezone.utc).isoformat(), "result": "PASS",
          "event_matrix": matrix, "security_permissions": {"contents": "read"},
          "hosted_schedule": "UNVERIFIED"}
(Path(__file__).parent / "c4-event-matrix.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps(result))
