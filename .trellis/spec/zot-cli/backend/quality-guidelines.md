# Quality Guidelines

`zot-cli` is the public surface. Compatibility, parse coverage, JSON envelope
stability, and safety around writes matter more than clever abstractions.

## Required Patterns

- Add clap parse coverage for new command surfaces in
  `cli.rs::parses_new_library_and_item_command_surfaces`.
- Keep global flags (`--json`, `--verbose`, `--profile`, `--library`) on the root `Cli`.
  `--library` only accepts `user` or `group:<id>` through `parse_library_scope`.
- Return `CommandOutput` from handlers for JSON success payloads.
  `CommandOutput::new` assembles the envelope, adding `count`, `total`,
  active profile, and `api_version`. Do not branch on `ctx.json` in command
  modules — the decision lives once inside `CommandOutput::new`.
- Preserve human output helpers in `format.rs` for table/text output rather
  than open-coding repeated printing in command modules.
- Offload blocking PDF backend calls through `util::run_pdf`.
- Keep workspace dependency declarations centralized. The
  `workspace_version_guard` integration test verifies root internal path
  dependencies and member `.workspace = true` inheritance.
- To unit-test a `zot-cli` command handler against a lower-level crate's fake
  HTTP server, add `test-support = []` to that crate's `[features]` (see
  `zot-remote` and `zot-desktop`) exposing a `with_base_url_for_tests`-style
  constructor, then depend on it only from `zot-cli`'s `[dev-dependencies]`
  with `features = ["test-support"]` — the plain `[dependencies]` entry stays
  untouched so `workspace_version_guard` still sees one centralized,
  unconditional internal dependency. Do not gate production behavior behind
  `test-support`; it exists only to expose a test constructor.
- Treat `skills/zot` as the canonical operator skill. Update
  `.agents/skills/zot` and `.claude/skills/zot` only through `_install-skills`;
  never hand-edit generated mirrors.

## Testing Requirements

- Run `cargo test -p zot-cli` for command, format, config, merge, and utility
  changes.
- Run `cargo test -p zot-cli --test workspace_version_guard` after manifest
  edits.
- Run `just ci` before finishing broad changes; it runs fmt, check, clippy, and
  tests in the repo-defined order, then `skills-check`.
- Run `just skills-check` after canonical skill edits. It compares relative
  file sets and bytes for both mirrors and runs drift fixtures covering
  content, missing-file, and extra-file failures.
- Add targeted tests close to behavior: parse tests in `cli.rs`, output
  envelope tests in `format.rs`, helper tests in `util.rs`, command logic tests
  in the owning command module.

## Code Example

Handlers return a `CommandOutput`; the json branch inside `CommandOutput::new`
assembles stable envelope metadata (`count`, `total`, active profile,
`api_version`) and the dispatch layer emits it:

```rust
let seed = Some(EnvelopeMetaSeed {
    count: Some(items.len()),
    total: Some(total),
});
CommandOutput::new(ctx, items, seed, |items| print_items(items))
```

## Safety Rules

- Follow `skills/zot/SKILL.md` for write actions. Dry-run preview is
  meaningful for merge and status-sync flows; do not describe preview as
  applied.
- `item merge`, `library duplicates-merge`, `library dedupe`, `sync update-status`, and
  `item import` require explicit `--confirm` or `--apply` to mutate.
- Dedupe confirmation is normal-confidence only by default. Do not add
  `--include-low-confidence` without a separate explicit risk authorization.
- For high-risk or batch writes, inspect or preview before applying.
- A `--confirm`/`--apply` flag only says the user wants to write; it does not
  say the target is currently writable. Re-check the target's writable state
  (e.g. dedupe's low-confidence gate, connector import's
  `editable`/`library_editable` check — see `connector.md`) inside the
  confirm branch, immediately before the call that would mutate, and fail
  closed if the check fails. Prove this with a test where the fake backend
  would error or hang on the unexpected call, not just a test that asserts
  the JSON output looks right — an assertion-only test still passes if the
  gate is accidentally removed but the output field is hardcoded.

## Scenario: `item tag batch` safety gate

### 1. Scope / Trigger

- Trigger: changing batch tag filters, limits, confirmation, result fields, or per-item writes.
- Why: a fuzzy local query fans out into non-transactional Web writes, so permission, ceiling,
  and partial-state evidence must be runtime-enforced.

### 2. Signatures

```text
zot item tag batch [--query Q] [--tag T]
  [--add-tag T]... [--remove-tag T]...
  [--limit 50] [--max-affected 50] [--confirm]
```

```rust
BatchTagWriter::add_tags(key, tags) -> Result<()>
BatchTagWriter::remove_tags(key, tags) -> Result<()>
```

### 3. Contracts

- No `--confirm`: local preview only; the writer factory must not run.
- `matched` is the full filter count; `affected` is the `--limit`-selected count;
  `sample_keys` contains at most 10 selected keys.
- Confirmed apply requires `affected <= max_affected`. Each add/remove per key is one operation.
- Apply output contains state, counts, successful `{key, operation}` entries, and failed
  `{key, operation, error: ErrorPayload}` entries. Failures do not stop later operations.

### 4. Validation & Error Matrix

| Condition | Code / result | Writer calls |
|---|---|---:|
| missing/blank filter | `batch-tags-filter` | 0 |
| missing/blank mutation | `batch-tags-op` | 0 |
| same tag added and removed | `batch-tags-conflict` | 0 |
| zero limit | `batch-tags-limit` | 0 |
| zero/ exceeded ceiling on confirm | `batch-tags-max-affected` | 0 |
| some remote operations fail | `state: partial` | remaining operations continue |
| all remote operations fail | `state: failed` | all planned operations attempted |

### 5. Good / Base / Bad Cases

- Good: preview 100 matches with `limit=20`, reports `affected=20`, then confirm stays under a
  reviewed ceiling and reports every operation.
- Base: zero selected matches confirms as `applied` with zero operation counts.
- Bad: constructing `ctx.remote()` before the preview branch, comparing the ceiling to unselected
  `matched`, aborting on the first `?`, or treating a successful envelope as full apply success.

### 6. Tests Required

- CLI parse coverage for `--confirm` and `--max-affected`.
- Validation table with stable codes and zero writer calls.
- Writer-factory sentinel tests for preview and exceeded ceiling.
- Fault injection across add/remove and multiple keys, asserting call order, continuation,
  state/counts, and nested `runtime-error` plus domain codes.
- Canonical skill mirror check and bilingual workflow examples.

### 7. Wrong vs Correct

Wrong:

```rust
let remote = ctx.remote()?;
for item in matches {
    remote.add_tags(&item.key, &tags).await?;
}
```

Correct:

```rust
let plan = build_plan(local_search)?;
if !confirm { return preview(plan); }
plan.enforce_max_affected()?;
apply_all_and_record(&writer, plan).await
```

## Review Checklist

- Does the command belong under existing `library`, `item`, `collection`,
  `workspace`, `sync`, or `config` surfaces?
- Does the JSON path emit a standard envelope and useful meta?
- Does the human path avoid raw JSON unless explicitly exporting JSON?
- Are command parse tests and focused behavior tests updated?
- Does the command respect local-read, connector-import, and Web mutation
  boundaries without fallback?
- If `skills/zot` changed, were mirrors regenerated and `just skills-check`
  run instead of editing mirror files directly?

## Scenario: Effective config options and doctor write capability

### 1. Scope / Trigger

- Trigger: changing `--json`, `--profile`, result `--limit`, config output fields, or doctor Web-write fields.
- Why: agent envelopes must describe the options actually used, while output defaults must not weaken write ceilings.

### 2. Signatures

```rust
AppConfig::into_effective(profile) -> (AppConfig, Option<String>)
Cli::resolve_effective_options(configured_limit) -> Result<(), ZotError>
AppContext { json, profile, config, .. }
```

Doctor Web-write payload:

```json
{"configured": true, "verified": false, "permissions": null,
 "last_error": null, "checked": "credentials-only"}
```

### 3. Contracts

- Explicit profile wins; otherwise envelope `meta.profile` contains the materialized default profile.
- `--json` enables JSON; configured `output-format=json` is the default for success, runtime errors, and protocol rejection.
- Configured `output-limit` fills only whitelisted read-result commands with no explicit limit.
- Index, dedupe, tag-batch, and sync limits remain command-owned safety/workload bounds.
- Doctor does not emit `available` for credential-only Web checks and does not claim verification.

### 4. Validation & Error Matrix

| Condition | Result |
|---|---|
| explicit/profile output limit zero | `config-value` before dispatch |
| configured JSON on server/raw command | `json-protocol-unsupported` before command I/O |
| profile init includes root-only key | `config-key`, no saved partial mutation |
| Web credentials exist but unprobed | `configured=true`, `verified=false` |

### 5. Good / Base / Bad Cases

- Good: default profile selects JSON and limit 17; an explicit `--limit 3` remains 3.
- Base: root config defaults to table and limit 50.
- Bad: replace every field named `limit`, report credentials as `available`, or keep only explicit CLI profile in metadata.

### 6. Tests Required

- Unit tests cover default/explicit profile, configured/explicit limits, zero rejection, and excluded write/index commands.
- Context Debug uses a secret canary.
- Doctor schema asserts all five fields and absence of `available`.
- JSON integration tests retain one-document success/error and protocol rejection.

### 7. Wrong vs Correct

Wrong:

```rust
ctx.profile = cli.profile.clone();
args.limit = config.output.limit; // also overwrites write ceilings
```

Correct:

```rust
let (config, profile) = raw.into_effective(cli.profile.as_deref());
cli.resolve_effective_options(config.output.limit)?; // exhaustive read whitelist
```
