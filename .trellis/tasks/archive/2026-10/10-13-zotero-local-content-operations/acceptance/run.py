"""Interactive real-Zotero acceptance; mutations use the unmodified zot binary.

Run from the repository root after setup-fixture.js in the isolated profile.
Each scenario saves CLI envelopes and API snapshots in the ignored runtime dir.
Authorization decisions must be made by the user in Zotero, never by this script.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import urllib.request

ROOT = next(path for path in Path(__file__).resolve().parents
            if (path / "Cargo.toml").is_file() and (path / "justfile").is_file())
RUNTIME = ROOT / "target/local-api-acceptance/01a134db"
FIXTURE = json.loads((RUNTIME / "fixture.json").read_text())
assert FIXTURE.get("server_id"), "Bind fixture.json to the verified isolated Server ID first"
ITEM = FIXTURE["item"]
COLS = FIXTURE["collections"]
OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}))
ENV = dict(os.environ, ZOT_DATA_DIR=str(RUNTIME / "data"), ZOT_API_KEY="")
BINARY = ROOT / "target/debug/zot.exe"
assert Path(FIXTURE["data_dir"]).resolve() == (RUNTIME / "data").resolve()


def read(path):
    request = urllib.request.Request("http://127.0.0.1:23119/api/" + path)
    request.add_header("Zotero-Server-ID", FIXTURE["server_id"])
    with OPENER.open(request, timeout=10) as response:
        server = response.headers["Zotero-Server-ID"]
        assert server and server != "2ZxitCcOtvOu", "Refusing original main library"
        assert server == FIXTURE["server_id"], "Instance changed"
        return json.load(response)


def snapshot():
    item = read(f"users/0/items/{ITEM}")
    assert item["data"]["extra"] == "Synthetic test fixture. Not a real publication."
    children = read(f"users/0/items/{ITEM}/children")
    assert {child["key"] for child in children} == set(FIXTURE["children"])
    return {"item": item, "children": sorted(children, key=lambda child: child["key"])}


def cli(args, expected_error=None):
    result = subprocess.run(
        [str(BINARY), "--json", *args], env=ENV, capture_output=True,
        text=True, encoding="utf-8", timeout=150,
    )
    output = json.loads(result.stdout)
    events.append({"args": args, "exit_code": result.returncode, "output": output})
    if expected_error:
        assert result.returncode == 1 and output["ok"] is False, output
        assert output["error"]["code"] == expected_error, output
        return output
    assert result.returncode == 0 and output["ok"] is True, output
    return output["data"]


def update(*args):
    return ["item", "update", ITEM, "--write-backend", "local", *args]


def move():
    return ["collection", "move-item", ITEM, "--from", COLS["A"],
            "--to", COLS["B"], "--write-backend", "local"]


def confirmed(args, preview):
    return [*args, "--confirm", "--expected-server-id", preview["server_id"],
            "--expected-version", str(preview["expected_version"])]


def apply(args, deny=False):
    preview = cli(args)
    assert preview["state"] == "preview" and preview["write_accepted"] is False
    print(json.dumps({"scenario": scenario, "preview": preview}, ensure_ascii=False), flush=True)
    if deny:
        cli(confirmed(args, preview), "local-api-forbidden")
    else:
        result = cli(confirmed(args, preview))
        assert result["state"] == "applied" and result["verified"] is True, result
        assert result["write_accepted"] is True, result


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("scenario", choices=[
    "preview", "deny", "fields", "creators", "move", "memberships",
    "overwrite", "save-stale", "reject-stale", "server-conflict", "after-restart",
])
scenario = parser.parse_args().scenario
events = []
before = snapshot()
report = {"scenario": scenario, "before": before, "events": events, "passed": False}
try:
    if scenario == "preview":
        assert cli(update("--field", "DOI=10.0000/cli-acceptance"))["state"] == "preview"
        kept = cli(update("--title", "Must not replace existing title"))
        assert kept["state"] == "unchanged" and kept["skipped_fields"] == ["title"]
        cli(update("--field", "parentItem=INVALID"), "local-write-field")
        assert cli(move())["state"] == "preview"
    elif scenario in ("deny", "fields"):
        apply(update("--field", "DOI=10.0000/cli-acceptance",
                     "--field", "abstractNote=Synthetic acceptance abstract.",
                     "--field", "conferenceName=CLI Acceptance Conference"),
              deny=scenario == "deny")
    elif scenario == "creators":
        apply(update("--creators-file", str(Path(__file__).with_name("creators.json"))))
    elif scenario == "move":
        assert set(before["item"]["data"]["collections"]) == {COLS["A"], COLS["C"]}
        apply(move())
        assert cli(move())["state"] == "unchanged"
    elif scenario == "memberships":
        for action in ("add-item", "remove-item"):
            args = ["collection", action, COLS["A"], ITEM, "--write-backend", "local"]
            apply(args)
            assert cli(args)["state"] == "unchanged"
    elif scenario == "overwrite":
        args = update("--field", "abstractNote=Explicitly replaced acceptance abstract.")
        assert cli(args)["state"] == "unchanged"
        apply([*args, "--overwrite"])
    elif scenario == "save-stale":
        args = update("--field", "publisher=Should not be written from stale preview")
        (RUNTIME / "stale.json").write_text(json.dumps({"args": args, "preview": cli(args)}))
    elif scenario == "reject-stale":
        stale = json.loads((RUNTIME / "stale.json").read_text())
        assert before["item"]["version"] != stale["preview"]["expected_version"]
        cli(confirmed(stale["args"], stale["preview"]), "local-api-conflict")
    elif scenario == "server-conflict":
        args = update("--field", "publisher=Should not be written to another instance")
        preview = cli(args)
        preview["server_id"] = "deliberately-wrong-server"
        cli(confirmed(args, preview), "local-api-conflict")
    elif scenario == "after-restart":
        apply(update("--date", "2026"))
    after = snapshot()
    report["after"] = after
    assert before["children"] == after["children"], "Child content or identity changed"
    if scenario in ("preview", "deny", "save-stale", "reject-stale", "server-conflict"):
        assert before == after, "Read-only or rejected operation changed data"
    if scenario in ("move", "memberships"):
        assert set(after["item"]["data"]["collections"]) == {COLS["B"], COLS["C"]}
        for field, value in before["item"]["data"].items():
            if field not in ("collections", "version", "dateModified"):
                assert after["item"]["data"].get(field) == value, field
    report["passed"] = True
    print(json.dumps({"scenario": scenario, "passed": True, "version": after["item"]["version"]}))
finally:
    (RUNTIME / f"{scenario}.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
