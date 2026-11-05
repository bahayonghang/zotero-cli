# Windows Zotero 10.0.6 acceptance

These are interactive task fixtures, not a test that runs unattended in CI.
The checked implementation is `16ce140`. Results are in `results.json`;
the detailed interpretation and remaining limits are in `../validation.md`.

## Isolation and preparation

- Use a fresh profile at `target/local-api-acceptance/01a134db/profile` and
  explicitly set its data directory to the sibling `data` directory. Never
  copy the main profile, library, or credentials. Leave sync unconfigured.
- Close the normal Zotero instance before starting the isolated profile with
  `zotero.exe -no-remote -profile <absolute-profile-path>`. The production CLI
  uses port 23119. Check the actual data directory and distinct Server ID.
- The user enables local communication in Zotero's Advanced settings and
  personally handles authorization dialogs. Do not automate those controls.
- Run `setup-fixture.js` from Tools → Developer → Run JavaScript in async mode.
  It checks the isolated directory and empty library, then creates A/B/C, one
  synthetic conference paper in A/C, one child note, and a linked attachment.
  The attachment is metadata pointing to example.invalid; no PDF is downloaded.
- Confirm the running instance and add its observed `server_id` to the generated
  `target/local-api-acceptance/01a134db/fixture.json`. The runner requires this
  binding and verifies the synthetic marker and both child keys before each run.
- Build `target/debug/zot.exe`, run its JSON doctor with process-local
  `ZOT_DATA_DIR` pointing at the test data directory, and confirm no Web key.
  Run scripts without Python's `-O` flag, because assertions are the checks.

## Sequence

From the repository root, use `python <this-directory>/run.py <scenario>`:

1. `preview`: field/move plans, existing title preservation, reserved field rejection.
2. `deny`: user selects Deny; require rejection and a completely unchanged snapshot.
3. `fields`: user selects Allow; fill synthetic DOI, abstract and conference name.
4. `creators`: user selects Always Allow; preserve personal/organization names,
   author/editor roles and array order. `creators.json` is the complete new list.
5. `move`, `memberships`, `overwrite`: A/C→B/C, add/remove A, repeated no-ops,
   preserve existing abstract by default, then explicitly replace it.
6. `server-conflict`: reject a deliberately wrong Server ID before authorization.
7. `save-stale`: save a publisher preview; edit the title through Zotero UI and
   leave the field so Zotero saves it. `reject-stale` must reject the old version.
8. Close and reopen the same test profile; `after-restart` fills the date using
   the saved credential without another authorization dialog. Inspect the UI.
9. Restore the normal Zotero instance. Verify its original Server ID. A confirmed
   request using the **test** Server ID must fail before reading/writing an item.

Each runner scenario writes envelopes and before/after API snapshots under
`target/local-api-acceptance/01a134db/`; children must remain identical. Stop at
the first failure. Never blindly rerun a scenario that may already have applied.

Remembered authorization uses the normal platform config directory. A restricted
agent sandbox may require an approved execution outside the filesystem sandbox
to persist it. The observed save-denial happened before PATCH; after checking the
unchanged item, a new preview/authorization succeeded. Do not print the key.

The first fixture run created all objects but failed to write its manifest due
to a Windows path separator error. The manifest was recovered using same-server
GETs; the saved fixture now uses `PathUtils` for that path. This fixture export
correction is not a product-code change.

## Evidence boundaries

`results.json` contains compact outcomes and hashes of the detailed local files.
It contains no authorization keys. The runtime profile, database, credential
files and full local snapshots stay untracked. Group permission errors and
submission timeout/unknown-result behavior have mock coverage; this acceptance
did not use a synced group or interrupt a real database commit. Undo, Better
BibTeX integration, Linux/macOS and MSRV execution were not tested here.
