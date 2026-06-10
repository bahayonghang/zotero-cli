# Quality Guidelines

`zot-cli` is the public surface. Compatibility, parse coverage, JSON envelope
stability, and safety around writes matter more than clever abstractions.

## Required Patterns

- Add clap parse coverage for new command surfaces in
  `cli.rs::parses_new_library_and_item_command_surfaces`.
- Keep global flags (`--json`, `--profile`, `--library`) on the root `Cli`.
  `--library` only accepts `user` or `group:<id>` through `parse_library_scope`.
- Use `print_enveloped` for JSON success payloads. It adds `count`, `total`,
  active profile, and `api_version`.
- Preserve human output helpers in `format.rs` for table/text output rather
  than open-coding repeated printing in command modules.
- Offload blocking PDF backend calls through `util::run_pdf`.
- Keep workspace dependency declarations centralized. The
  `workspace_version_guard` integration test verifies root internal path
  dependencies and member `.workspace = true` inheritance.

## Testing Requirements

- Run `cargo test -p zot-cli` for command, format, config, merge, and utility
  changes.
- Run `cargo test -p zot-cli --test workspace_version_guard` after manifest
  edits.
- Run `just ci` before finishing broad changes; it runs fmt, check, clippy, and
  tests in the repo-defined order.
- Add targeted tests close to behavior: parse tests in `cli.rs`, output
  envelope tests in `format.rs`, helper tests in `util.rs`, command logic tests
  in the owning command module.

## Code Example

Keep JSON envelope metadata stable:

```rust
let meta = EnvelopeMeta {
    count: seed.count,
    total: seed.total,
    profile: ctx.profile.clone(),
    api_version: Some(ENVELOPE_API_VERSION),
};
print_json(&CliEnvelope::ok_with_meta(data, meta))
```

## Safety Rules

- Follow `skills/zot-skills/SKILL.md` for write actions. Dry-run preview is
  meaningful for merge and status-sync flows; do not describe preview as
  applied.
- `item merge`, `library duplicates-merge`, and `sync update-status` require
  explicit `--confirm` or `--apply` to mutate.
- For high-risk or batch writes, inspect or preview before applying.

## Review Checklist

- Does the command belong under existing `library`, `item`, `collection`,
  `workspace`, `sync`, or `config` surfaces?
- Does the JSON path emit a standard envelope and useful meta?
- Does the human path avoid raw JSON unless explicitly exporting JSON?
- Are command parse tests and focused behavior tests updated?
- Does the command respect local-read vs remote-write boundaries?
