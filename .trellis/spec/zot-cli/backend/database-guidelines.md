# Database Guidelines

`zot-cli` does not write SQL directly. It chooses the exact supported read or
write transport and delegates work to the owning crate.

## Access Boundaries

- Use `ctx.local_library()?` for local Zotero reads through `zot-local`.
- Use `ctx.remote()?` for Zotero Web API writes through `zot-remote`.
- Use `ctx.connector()?` only for built-in connector import and `/api/`
  readiness probes. Connector errors never fall back to `ctx.remote()`.
- Use workspace stores from `zot-local` for workspace TOML/index operations.
- Use `ctx.library_index_path()` for the library-wide semantic index path.
- Use `ctx.pdf_backend()` (`Arc<dyn PdfBackend>`) for PDF extraction; only
  `AppContext::from_cli` and `doctor` construct `PdfiumBackend` directly
  (doctor reports Pdfium-specific diagnostics via inherent `status()`).
- Never open `zotero.sqlite` or sidecar SQLite directly from command modules.

## Narrow Trait Consumption

Read-side code should depend on the narrow per-data-domain traits from
`zot-local` (`ItemReader`, `CollectionNav`, `CollectionContent`, `NoteReader`,
`AttachmentSource`; engine side: `RagLibrary`) rather than concrete
`LocalLibrary`. This does not move the access boundary — `ctx.local_library()?`
stays the only production entry point — it makes the seam replaceable:

- Split read arms into an inner `fn handle_read<L: Trait>(ctx, library: &L, cmd)`
  and keep write arms (which go through `ctx.remote()`) in the outer `handle`.
  Tests then drive the full output path with a fake library
  (`collection.rs`, `item/note.rs` are the reference implementations).
- Shared helpers take `&impl Trait` (`util::require_item` takes
  `&impl ItemReader`, `require_pdf_attachment` takes `&impl AttachmentSource`).
- New traits mirror `db.rs` inherent signatures exactly and live in
  `zot-local/src/library_traits.rs`; do not move SQL there.

## Doctor Gate

Operational guidance in `AGENTS.md` and `skills/zot/SKILL.md` requires
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

Doctor reports `local_sqlite_read`, `local_http_read`, `connector_write`, and
`web_write` independently. Web credentials do not represent connector import
capability, and Local HTTP remains read-only.

## Write Boundaries

Command handlers can orchestrate two distinct write paths:

- `item import` is the only connector write. Dry-run validates the selected
  target; confirm rechecks writability immediately before import.
- `item merge`, `library duplicates-merge`, `library dedupe`, and all other
  current mutations use the Web API. Remote mutation logic belongs in
  `zot-remote`.

Examples:

- `item note add/update/delete` calls `ctx.remote()?`.
- `collection create/rename/delete/add-item/remove-item` calls `ctx.remote()?`.
- `sync update-status --apply` calls `ctx.remote()?` only when apply is set.
- Merge commands return a preview unless `--confirm` is set.
- `library dedupe --confirm` skips low-confidence groups before writer calls
  unless the user explicitly supplied `--include-low-confidence`.

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
- Do not describe Zotero Local HTTP as a write transport.
- Do not add automatic connector/Web fallback around import errors.
- Do not make `--json` subcommand-local. It is a global flag and must be
  parsed before the subcommand.
- Do not treat saved searches as result snapshots; they are remote query
  condition objects.
