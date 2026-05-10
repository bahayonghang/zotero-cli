# Performance and behaviour limits

This page documents the operational ceilings and corner-case semantics that
agent runs (and humans) need to know before using `zot` against larger
libraries. Keep it accurate as the implementation evolves.

## Semantic search complexity

- `zot library semantic-search` and `zot workspace query` both perform an
  **O(N) full scan** of indexed chunks for every query (cosine similarity over
  every embedding row in the relevant SQLite index).
- Comfortable upper bound: **≤ 10,000 chunks per index**. At that size a
  cold-cache query still completes in well under one second on commodity
  hardware. Beyond that the scan starts to dominate; consider scoping queries
  to a smaller workspace or filtering by collection before running
  `semantic-index`.
- The library-wide index lives in `~/.config/zot/indexes/<scope>.idx.sqlite`;
  per-workspace indexes are sidecars next to the workspace TOML
  (`<name>.idx.sqlite`).
- Approximate-nearest-neighbour search (e.g. HNSW) is **not** implemented and
  is intentionally deferred to a future minor release.

## Embedding service batching

- `EmbeddingClient::embed` batches inputs internally with a **default batch
  size of 64**. Override with `ZOT_EMBEDDING_BATCH_SIZE`. Values that fail to
  parse, or `0`, fall back to the default.
- Batches are dispatched **serially** to avoid tripping rate limits on
  shared embedding endpoints. Output order is preserved across batches.
- The total embedding count is still validated against the input count, so a
  partial batch failure raises `embedding-count-mismatch` instead of silently
  truncating.

## SQLite `LIKE` search semantics

- All user-provided text used in `LIKE` searches (general search, notes,
  annotations, creator filter) is escaped with `escape_like` and the SQL adds
  `ESCAPE '\'`. Concretely:
  - `%` matches a literal percent sign.
  - `_` matches a literal underscore.
  - `\` matches a literal backslash.
- That means a query like `50%` will only match strings that contain
  `50%` literally, and `foo_bar` will only match `foo_bar` (not `fooXbar`).

## Scite batch endpoints

- `SciteClient::get_reports_batch` calls Scite's `/tallies` and `/papers`
  endpoints in chunks of **500 DOIs**. Inputs over 500 are no longer silently
  truncated (the historical `take(500)` pattern); larger inputs fan out
  across multiple HTTP requests and the merged result preserves every DOI.

## PDF outline depth

- `extract_outline` derives an outline level by walking a bookmark's
  `parent()` chain via `pdfium-render`'s API rather than counting dots in the
  title. Top-level bookmarks resolve to `level == 1`; numeric prefixes such
  as `3.2.1` or labels like `Fig. 1` no longer skew the depth.

## PDF text annotation creation

- `zot item annotation create --text "<phrase>"` accepts an optional
  `--occurrence N` flag (default `1`). When the same phrase appears multiple
  times on the same page, agents can target the Nth match instead of being
  forced onto the first occurrence.
- The JSON response includes `occurrence`, `total_matches`, and (when
  applicable) `more_occurrences`, which makes it straightforward to chain
  follow-up calls.

## CrossRef polite-pool contact email

- CrossRef and Unpaywall requests both include a `mailto:` contact in the
  `User-Agent` / query-string. Set `ZOT_CONTACT_EMAIL` to your real address
  to enter the CrossRef polite pool. The default is `noreply@zot.local`,
  which is recognised as an opaque placeholder rather than a real mailbox.

## JSON envelope contract

- `zot --json ...` always returns the standard envelope from
  `zot-core::CliEnvelope`. Success payloads now carry
  `meta.profile == "<active>"` and `meta.api_version == 1` regardless of
  which command produced the output. Treat `api_version` as the cut-off
  marker for the 0.5.0 contract; if it is absent or larger, expect schema
  drift.
