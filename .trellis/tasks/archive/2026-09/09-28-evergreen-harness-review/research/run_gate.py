"""Record a local verification command without changing repository inputs."""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent

def snapshot():
    paths = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
    paths += subprocess.check_output(
        ["git", "ls-files", "--others", "--exclude-standard", "-z", "--", "scripts/tests"],
        cwd=ROOT).decode().split("\0")
    hashes = {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest()
              for p in paths if p and (ROOT / p).is_file()}
    return hashlib.sha256(json.dumps(hashes, sort_keys=True).encode()).hexdigest()

label, *command = sys.argv[1:]
if not command:
    raise SystemExit("usage: run_gate.py LABEL COMMAND [ARG ...]")
resolved = shutil.which(command[0])
if resolved:
    command[0] = resolved
record = {"command": command, "cwd": str(ROOT),
          "started_utc": datetime.now(timezone.utc).isoformat(),
          "baseline_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
          "input_tree_sha256": snapshot(),
          "lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
          "environment_overrides": {k: os.environ[k] for k in ("CARGO_TARGET_DIR", "CARGO_NET_OFFLINE") if k in os.environ}}
with (OUT / f"{label}.log").open("wb") as stream:
    result = subprocess.run(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
record.update(exit_code=result.returncode, ended_utc=datetime.now(timezone.utc).isoformat(),
              output_tree_sha256=snapshot(), log=f"{label}.log")
(OUT / f"{label}.result.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
print(json.dumps(record))
raise SystemExit(result.returncode)
