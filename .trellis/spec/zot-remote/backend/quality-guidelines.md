# Quality Guidelines

Remote-client changes should preserve service-specific limits, batching, and
write safety. Prefer small pure helpers and tests for parsing/normalization
over broad live-network tests.

## Required Patterns

- Reuse `HttpRuntime` for all clients so connection pooling and timeout policy
  stay centralized.
- Validate credentials that become headers before storing them in clients.
  `ZoteroRemote::new` and `SemanticScholarClient::new` both validate header
  values.
- Batch external service calls according to documented limits. Embeddings use
  `ZOT_EMBEDDING_BATCH_SIZE` with default 64; Scite batches DOI lists in
  chunks of 500.
- Preserve output count/order for embedding batches. `EmbeddingClient::embed`
  extends batches serially and validates the final vector count.
- Map responses through the shared `http.rs` layer (`remote_err`, `http_hint`,
  `ensure_status`, `read_json`, `ensure_empty`) instead of redefining error
  mapping per client. Intentional divergences: soft-fail lookups that return
  `Ok(None)` on non-success, and the attachment upload's exact-201 check.
- Every client's base URL is overridable via env for tests and local
  substitutes, with production defaults unchanged: `ZOT_BBT_URL`/`ZOT_BBT_PORT`,
  `ZOT_SCITE_API_BASE`, `ZOT_CROSSREF_API_BASE`, `ZOT_UNPAYWALL_API_BASE`,
  `ZOT_PMC_API_BASE`, `ZOT_SEMANTIC_SCHOLAR_GRAPH_BASE` (OA PDF endpoint),
  `ZOT_SEMANTIC_SCHOLAR_API_BASE`, and `ZOT_ZOTERO_API_BASE`; the embedding
  endpoint comes from `EmbeddingConfig`.

## Testing Requirements

- Add unit tests for normalization and pure merge logic, as seen in
  `oa.rs`, `embedding.rs`, `scite.rs`, and `semantic_scholar.rs`.
- For request shape and response mapping, drive clients against the loopback
  fake server (`test_support::spawn_server`) through their `#[cfg(test)]`
  `with_base_url` constructors. It binds `127.0.0.1:0`, scripts responses in
  memory, and captures requests for header/method assertions — this is the
  recognized local adapter; never hit a live service in tests.
- For batching changes, test chunk counts and total coverage rather than
  relying on live services. Existing tests cover 200 embedding inputs and 600
  Scite DOIs.
- For Zotero write orchestration, keep side effects behind `ZoteroRemote`
  methods and validate dry-run/preview behavior in `zot-cli` command tests.

## Code Example

Embedding count validation prevents silent truncation:

```rust
if requested == 0 || embeddings.len() == requested {
    return Ok(embeddings);
}

Err(ZotError::Remote {
    code: "embedding-count-mismatch".to_string(),
    message: format!(
        "Embedding service returned {} vectors for {} inputs",
        embeddings.len(),
        requested
    ),
    hint: Some("Check embedding service health or response format".to_string()),
    status: None,
})
```

## Operational Limits

- `docs/agents/limits.md` records the embedding default batch size of 64 and
  Scite batch size of 500 DOIs.
- CrossRef and Unpaywall requests should include polite contact information
  through `ZOT_CONTACT_EMAIL`; `oa.rs` defaults to `noreply@zot.local`.

## Review Checklist

- Does every mutating Zotero request carry the right version precondition or
  write token?
- Does a new external call reuse `HttpRuntime` and map errors into
  `ZotError::Remote`?
- Are batching and count-preservation rules tested?
- Are optional service misses intentionally represented as `Ok(None)`?
