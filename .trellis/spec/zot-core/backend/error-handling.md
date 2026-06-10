# Error Handling

`zot-core` defines the error contract for the workspace. Other crates should
return `ZotResult<T>` and map failures into `ZotError` variants with stable
machine-readable codes.

## Error Types

- `ZotError::InvalidInput` is for CLI/config/user validation failures such as
  invalid `--library` values in `parse_library_scope`.
- `ZotError::Io` is for filesystem operations and must include the path.
- `ZotError::ConfigParse` is for TOML decode/encode failures.
- `ZotError::Database` is for SQLite and local sidecar index failures from
  `zot-local`.
- `ZotError::Remote` is for Zotero Web API, enrichment APIs, and embedding
  service failures from `zot-remote`.
- `ZotError::Pdf` is for PDF extraction, Pdfium setup, and annotation geometry.
- `ZotError::Unsupported` is for deliberately unavailable features such as
  command surfaces that are scaffolded but not implemented.

## Error Payload Contract

`ZotError::payload()` reduces every variant to:

```rust
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    pub hint: Option<String>,
}
```

The CLI wraps this in `CliEnvelope::Err` for `--json` output. Preserve this
shape unless intentionally changing the JSON API version in `zot-cli`.

## Patterns

- Prefer stable kebab-case-ish codes such as `invalid-library`,
  `config-parse`, `embedding-count-mismatch`, and `db-not-found`.
- Include a `hint` when the user can take a specific action, for example
  `parse_library_scope` returns `Use 'user' or 'group:<id>'`.
- Map source errors at the boundary where context is still available. Config
  reads in `AppConfig::load_raw` include the path; HTTP clients in
  `zot-remote` include the request context code and optional status.

## Code Example

`parse_library_scope` validates the public library grammar in `zot-core` before
`zot-cli` creates an `AppContext`:

```rust
Err(ZotError::InvalidInput {
    code: "invalid-library".to_string(),
    message: format!("Invalid library scope: {value}"),
    hint: Some("Use 'user' or 'group:<id>'".to_string()),
})
```

## Avoid

- Do not return raw `std::io::Error`, `rusqlite::Error`, `reqwest::Error`, or
  parser errors across crate boundaries.
- Do not put secrets such as API keys into error messages or hints.
- Do not add error variants unless existing categories cannot represent the
  failure without losing important routing information.
