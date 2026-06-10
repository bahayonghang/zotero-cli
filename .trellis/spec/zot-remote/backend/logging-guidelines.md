# Logging Guidelines

`zot-remote` clients should be output-free. They return typed data or
`ZotError` values, and `zot-cli` decides whether to print human text or JSON
envelopes.

## Diagnostics Pattern

- Put operation context in error codes such as `crossref-request`,
  `embedding-json`, `scite-papers`, or `update-item`.
- Include HTTP status in `ZotError::Remote.status` when available.
- Use hints only for actionable user/configuration steps, not for generic
  commentary.

## What Not To Expose

- Zotero API keys, Semantic Scholar keys, embedding bearer tokens, upload keys,
  and raw authorization responses.
- Full request/response bodies that could contain private Zotero metadata,
  except short body text already included in remote HTTP error messages when
  diagnosing a failed request.
- Attachment file bytes or local paths beyond the path needed for an IO error.

## Code Example

Remote clients should not print progress inside loops. `SciteClient` batches
internally and returns a merged `BTreeMap`:

```rust
for chunk in dois.chunks(SCITE_BATCH_SIZE) {
    let response = self.client.post(format!("{}/papers", self.base_url)).send().await?;
    ...
    merged.extend(payload.papers);
}
```

If progress is needed for a CLI workflow, expose counts in the return payload
and print from `zot-cli`.
