# Database Guidelines

`zot-cli` does not write SQL directly. It chooses the correct runtime path and
delegates database or remote work to the owning crate.

## Access Boundaries

- Use `ctx.local_library()?` for local Zotero reads through `zot-local`.
- Use `ctx.remote()?` for Zotero Web API writes through `zot-remote`.
- Use workspace stores from `zot-local` for workspace TOML/index operations.
- Use `ctx.library_index_path()` for the library-wide semantic index path.
- Never open `zotero.sqlite` or sidecar SQLite directly from command modules.

## Doctor Gate

Operational guidance in `AGENTS.md` and `skills/zot-skills/SKILL.md` requires
`zot --json doctor` first for:

- New environments.
- Any write action.
- PDF extraction, outline, annotation, or attachment issues.
- Workspace indexing/query issues.
- Citation key lookup and saved search/config troubleshooting.

If `zot` is not installed in a development checkout, use:

```text
cargo run -q -p zot-cli -- --json doctor
```

Use one invocation path consistently during an agent session.

## Write Boundaries

Command handlers can orchestrate Web API writes, but remote mutation logic
belongs in `zot-remote`. Examples:

- `item note add/update/delete` calls `ctx.remote()?`.
- `collection create/rename/delete/add-item/remove-item` calls `ctx.remote()?`.
- `sync update-status --apply` calls `ctx.remote()?` only when apply is set.
- Merge commands return a preview unless `--confirm` is set.

## Code Example

`AppContext::remote` validates configured write credentials before constructing
`ZoteroRemote`:

```rust
if self.config.zotero.api_key.is_empty() {
    return Err(zot_core::ZotError::InvalidInput {
        code: "write-credentials".to_string(),
        message: "Missing Zotero API key".to_string(),
        hint: Some("Run `zot config init` or set ZOT_API_KEY".to_string()),
    });
}
```

## Avoid

- Do not touch `zotero.sqlite` for writes.
- Do not make `--json` subcommand-local. It is a global flag and must be
  parsed before the subcommand.
- Do not treat saved searches as result snapshots; they are remote query
  condition objects.
