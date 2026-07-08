# Directory Structure

`zot-remote` is a set of focused HTTP clients sharing one `HttpRuntime`.
Each module should own one external service or remote capability.

## Directory Layout

```text
src/zot-remote/src/
├── better_bibtex.rs       # Better BibTeX citation-key lookup
├── embedding.rs           # Embedding service client and batching
├── http.rs                # Shared reqwest client runtime
├── lib.rs                 # Public exports
├── oa.rs                  # CrossRef, arXiv, Unpaywall, PMC, OA PDF resolution
├── scite.rs               # Scite reports and batch endpoints
├── semantic_scholar.rs    # Preprint publication-status checks
├── test_support.rs        # Test-only loopback fake HTTP server (cfg(test))
└── zotero.rs              # Zotero Web API writes, saved searches, attachments
```

## Module Ownership

- `http.rs` builds the shared `reqwest::Client` with connect/request timeouts
  and the `zot-cli/<version>` user agent, and owns the shared response layer
  (`remote_err`, `http_hint`, `ensure_status`, `read_json`, `ensure_empty`).
- `zotero.rs` owns Zotero Web API endpoints, API-key headers, version
  preconditions, write tokens, attachment upload authorization/register flow,
  saved searches, and flat editable object updates.
- `embedding.rs` owns embedding endpoint configuration, serial batching, and
  response count validation.
- `oa.rs` owns DOI/arXiv normalization and metadata/PDF resolution through
  CrossRef, arXiv, Unpaywall, Semantic Scholar, and PMC.
- `scite.rs` owns Scite tally/paper report calls and 500-DOI batch fan-out.
- `semantic_scholar.rs` owns preprint ID extraction and publication-status
  checks.

## Naming Conventions

- Client structs use `<Service>Client`, except Zotero writes use
  `ZoteroRemote`.
- Small normalized data types are public only when the CLI or another crate
  consumes them, such as `CrossRefWork`, `ArxivWork`, `ResolvedPdfUrl`,
  `PreprintInfo`, and `PublicationStatus`.
- Request helper functions should carry operation names that become error
  codes, for example `remote_err("embedding-request")`.

## Code Example

Construct remote clients from the shared runtime rather than creating fresh
`reqwest::Client` instances:

```rust
pub fn new(runtime: &HttpRuntime, config: EmbeddingConfig) -> Self {
    Self {
        client: runtime.client_clone(),
        config,
    }
}
```

`HttpRuntime` documents why: cloning `reqwest::Client` reuses an internal
connection pool.

## Avoid

- Do not put CLI confirmation, dry-run wording, or output formatting here.
- Do not read `zotero.sqlite`; remote clients use Web APIs only.
- Do not add per-request client construction unless a service truly requires
  isolated client configuration.
