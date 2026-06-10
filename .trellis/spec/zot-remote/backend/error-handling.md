# Error Handling

`zot-remote` maps network, HTTP status, JSON, and remote validation failures to
`ZotError::Remote` or `ZotError::InvalidInput`.

## Error Mapping

- Use `ZotError::InvalidInput` before sending a request when local validation
  fails, such as invalid API-key header values, missing DOI/URL for item
  creation, missing item key/version in flat update payloads, or unconfigured
  embedding service.
- Use `ZotError::Remote` for HTTP request errors, non-success HTTP statuses,
  remote JSON decode failures, and unexpected response shapes.
- Use `ZotError::Io` only for local file reads needed by remote operations,
  such as attachment upload bytes and metadata in `authorize_attachment_upload`.

## Response Handling

- Zotero API helpers centralize response handling in `ensure_empty` and
  `ensure_json`.
- Most service modules have a small `remote_err(code)` helper that preserves
  the operation code and optional HTTP status from `reqwest::Error`.
- Non-critical lookups may return `Ok(None)` on remote miss. Examples:
  Scite single report calls return `Ok(None)` when both tally and paper are
  absent; Semantic Scholar publication checks return `Ok(None)` for 404.

## Code Example

HTTP status failures should keep the status and remote body:

```rust
return Err(ZotError::Remote {
    code: code.to_string(),
    message: format!("Request failed with status {}: {body}", status.as_u16()),
    hint: http_hint(Some(status)),
    status: Some(status.as_u16()),
});
```

## Hints

`zotero.rs::http_hint` currently maps:

- `403` to API key write-access guidance.
- `412` to "Object changed remotely; re-fetch before retrying".
- `428` to missing version/precondition guidance.
- `409` to target-library locked guidance.

## Avoid

- Do not unwrap response bodies or JSON fields in runtime code.
- Do not hide partial remote failures by returning success with missing data
  unless the source is explicitly optional and the caller can proceed.
- Do not include API keys, bearer tokens, or upload authorization secrets in
  error messages.
