# Changelog

All notable changes to this project will be documented in this file. Dates use
`YYYY-MM-DD`. Versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- `item merge`, `library duplicates-merge`, and `library dedupe` now use the
  Zotero Web API exclusively. Configure `library_id` and `api_key` before
  confirming these operations.
- Zotero Desktop's built-in connector is now the only local write path and is
  limited to importing new BibTeX/RIS records into the selected writable target.
- JSON payloads no longer include `write_backend`,
  `selected_write_backend`, or `capabilities.desktop_write`. This breaking
  schema change keeps `meta.api_version = 1`; consumers must follow the
  changelog for field additions and removals within the 0.x release line.

### Removed

- Removed the bundled `zot-bridge` XPI, `zot bridge ...`, the global
  `--write-backend` flag, and `zot config set write-backend`.

### Migration

- Uninstall Zot Bridge from Zotero and remove legacy `desktop_bridge` and
  `write_backend` entries from `~/.config/zot/config.toml`. Old entries remain
  load-compatible and `zot --json doctor` reports one migration hint without
  exposing stored token values.

## [0.5.0] - 2026-05-10

This release closes 16 findings from the internal `CODE_REVIEW.md` audit,
covers them all with regression tests, and locks the JSON envelope contract
behind `meta.api_version = 1` for downstream consumers.

### Fixed

- **F-1**: CrossRef `abstract_field` now uses `#[serde(rename = "abstract")]`,
  so the abstract on every `zot item add-by-doi`/`fetch_crossref_work`
  response is populated instead of silently being `None`.
- **F-12**: The temporary fallback path in `LocalLibrary::connect` reopens the
  copied SQLite database with `OpenFlags::SQLITE_OPEN_READ_ONLY`, matching the
  primary URI-based read-only invariant.

### Changed

- **F-4 (minor)**: Every JSON envelope now carries `meta.profile = "<active>"`
  and `meta.api_version = 1`. The new `EnvelopeMetaSeed { count, total }` lets
  callers populate just the fields they care about; profile and api_version
  are injected by `print_enveloped(ctx, ...)`. Treat `api_version = 1` as the
  contract cut-off for 0.5.0.
- **F-5 (minor)**: `zot workspace index <name>` now performs **incremental**
  reindexing by default (only newly added items are re-embedded). Two new
  flags expose the previous behaviour and let callers turn off PDF fulltext:
  - `--force-rebuild` clears and rebuilds the entire workspace index.
  - `--no-fulltext` skips PDF text extraction and only embeds metadata.
- **F-8**: Scite's `/tallies` and `/papers` batch endpoints are now invoked in
  500-DOI chunks instead of being silently truncated to the first 500 DOIs.

### Added

- **F-10 (minor)**: `zot item annotation create` accepts `--occurrence N`
  (default `1`). The JSON response carries `occurrence`, `total_matches`, and
  `more_occurrences`, so agents can tell when there are still more matches
  on the page.
- **F-16**: `docs/agents/limits.md` documents performance ceilings (semantic
  search O(N), embedding batch size, LIKE escape semantics, Scite chunking,
  PDF outline depth, polite-pool email, envelope `api_version`). Linked
  from `AGENTS.md` under "Agent skills".

### Performance

- **F-2**: `ZoteroRemote::upload_attachment` now reads the attachment file
  exactly once. `authorize_attachment_upload` returns
  `(FileUploadAuthorization, Vec<u8>)` and the caller reuses those bytes
  for the upload body instead of issuing a second `tokio::fs::read`.
- **F-3**: All `PdfBackend::*` calls in the command layer are wrapped in a
  new `run_pdf` helper that defers the synchronous PDF work to
  `tokio::task::spawn_blocking`. Single-threaded tokio runtimes (and the
  managed `pdfium` download path) no longer risk stalling the worker
  thread during the first PDF extraction.
- **F-6**: `extract_preprint_info` lifts its four arXiv regexes into a static
  `ARXIV_PATTERNS: Lazy<[Regex; 4]>` instead of recompiling them on every
  call.
- **F-15**: `EmbeddingClient::embed` now batches inputs (default 64,
  configurable via `ZOT_EMBEDDING_BATCH_SIZE`) and dispatches the batches
  serially. Total count validation is preserved across batches.

### Removed

- **F-13**: `zot-core` no longer pulls in `chrono`, `uuid`, or `serde_json`
  (none of them were used inside the crate). The workspace-level entries
  remain so other crates can opt back in cheaply.
- **F-14**: `zot-cli` no longer depends on `rmcp`. `zot mcp serve` still
  responds with `mcp-not-implemented`; the dependency will return when MCP
  lands.

### Documentation

- **F-7**: HTTP requests against CrossRef, Unpaywall, and PubMed Central now
  send `User-Agent: zot-cli/0.5.0 (mailto:<contact>)` plus a polite-pool
  `email=` parameter. The contact address falls back to the placeholder
  `noreply@zot.local` when `ZOT_CONTACT_EMAIL` is unset; setting that env
  var is what graduates a deployment into the CrossRef polite pool.
- **F-9**: Outline level for PDF bookmarks is derived from
  `pdfium_render::PdfBookmark::parent()` chains rather than `'.'` counts in
  the title, so structured headings like `3.2.1` and figure labels like
  `Fig. 1` no longer skew the level.
- **F-11**: Every user-provided LIKE pattern is escaped with `escape_like`
  and the SQL pairs `LIKE ? ESCAPE '\\'`. Searching for `50%` or `foo_bar`
  now matches the literal characters instead of treating them as
  wildcards.

### Notes for downstream consumers

- The envelope schema is the only behavioural cut-off for 0.5.0. Look for
  `meta.api_version = 1` in JSON output. Older releases emitted `meta == null`
  for many commands; the new contract is to always emit a meta block with at
  least `profile` and `api_version`.
- `zot workspace index` defaulting to incremental is a behavioural change.
  Pipelines that relied on every run rebuilding the index should pass
  `--force-rebuild` explicitly.
- `zot item annotation create` with multiple matches on the same page now
  defaults to the first occurrence (unchanged), but exposes `--occurrence`
  and richer JSON output for chained tooling.
