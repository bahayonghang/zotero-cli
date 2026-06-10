# Quality Guidelines

Most `zot-local` regressions are data-shape, query, or sidecar-index
regressions. Add narrow tests around the behavior being changed.

## Required Patterns

- Preserve primary-item filtering. `db.rs` excludes `attachment`, `note`, and
  `annotation` for top-level library searches; `search_regression.rs` verifies
  this for empty search, field search, list, and recent items.
- Preserve literal `LIKE` semantics. `escape_like` has unit tests for `%`, `_`,
  and `\`; new search paths must use the same helper.
- Preserve deterministic ordering after broad set collection. Existing code
  sorts items by explicit sort fields or key and sorts workspaces by name.
- Preserve sidecar index integrity. Use `with_write_tx` for multi-step RAG
  writes, invalidate cached BM25 stats after chunk mutations, and validate
  embedding counts/dimensions before writeback.
- Keep heavy PDF work behind the `PdfBackend` trait so CLI code can offload it
  and tests can use focused fakes.

## Testing Requirements

- Run `cargo test -p zot-local` for local data-layer changes.
- Add fixture-backed tests in `src/zot-local/tests/search_regression.rs` for
  Zotero schema/query behavior.
- Add sidecar-index tests in `src/zot-local/tests/semantic_index.rs` or inline
  `workspace.rs` tests for index schema, migration, and query behavior.
- Add PDF-specific unit tests in `pdf.rs` when changing Pdfium setup, cache
  paths, archive extraction, page range behavior, or annotation geometry.

## Code Example

`WorkspaceStore::save` writes through a temporary file and syncs before
persisting, with a Windows replacement path:

```rust
let mut temp = tempfile::NamedTempFile::new_in(&self.root)?;
temp.write_all(raw.as_bytes())?;
temp.as_file_mut().sync_all()?;
#[cfg(target_os = "windows")]
if path.exists() {
    std::fs::remove_file(&path)?;
}
temp.persist(&path)?;
```

Keep this atomic-save style for workspace TOML updates.

## Operational Limits

- `docs/agents/limits.md` documents semantic search as an O(N) scan over
  indexed chunks, with a comfortable ceiling around 10,000 chunks per index.
- `chunk_text` currently uses 500 max tokens with 50-token overlap for library
  and workspace indexing.
- Workspace names must match `^[a-z0-9]+(-[a-z0-9]+)*$`.

## Review Checklist

- Does the change keep Zotero's main database read-only?
- Are user search strings escaped before `LIKE`?
- Are sidecar schema changes migrated and covered by reopen tests?
- Are PDF failures typed as `ZotError::Pdf`?
- Did the change preserve the documented workspace file/index/cache layout?
