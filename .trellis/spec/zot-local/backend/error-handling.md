# Error Handling

`zot-local` returns `zot_core::ZotResult<T>` and maps local failures to
`ZotError::Database`, `ZotError::Pdf`, `ZotError::Io`, or
`ZotError::InvalidInput`.

## Error Mapping

- Use `ZotError::Database` for `rusqlite` and local sidecar schema/query
  failures. Helper functions `sql_err` in `db.rs` and `db_err` in
  `workspace.rs` keep context codes stable.
- Use `ZotError::Pdf` for Pdfium setup, PDF open/page/text/search failures,
  invalid PDF page ranges, and PDF annotation geometry problems.
- Use `ZotError::Io` for filesystem operations, including workspace TOML saves,
  temp file persistence, PDF cache directories, and attachment paths.
- Use `ZotError::InvalidInput` for local validation such as invalid workspace
  names or embedding count mismatches before writeback.

## Patterns

- Include operation-specific codes such as `db-not-found`, `search-notes`,
  `rag-open`, `rag-schema`, `pdf-open`, `pdf-cache-put`, and
  `invalid-workspace-name`.
- Preserve the path when mapping filesystem errors.
- Treat missing optional Zotero tables as empty results when that matches
  Zotero version compatibility. For example, annotation reads return an empty
  list if `itemAnnotations` is absent.
- Keep user-fixable setup failures actionable. `PdfiumBackend::status` and
  Pdfium errors mention `ZOT_PDFIUM_LIB_PATH` / `PDFIUM_LIB_PATH` when manual
  setup is needed.

## Code Example

Use the local SQL error helper when no path context is available:

```rust
fn sql_err(context: &'static str) -> impl Fn(rusqlite::Error) -> ZotError {
    move |source| ZotError::Database {
        code: context.to_string(),
        message: source.to_string(),
        hint: None,
    }
}
```

## Avoid

- Do not expose raw `rusqlite::Error` or `pdfium_render` errors across crate
  boundaries.
- Do not panic for runtime misses; unknown item keys should usually become
  `Ok(None)` at the local layer and a CLI-level not-found error where needed.
- Do not collapse PDF setup failures into generic database or IO errors; the
  CLI doctor command depends on distinguishing PDF availability.
