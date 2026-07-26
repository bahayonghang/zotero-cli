# Changelog

All notable changes to this project will be documented in this file. Dates use
`YYYY-MM-DD`. Versions follow [Semantic Versioning](https://semver.org/).

## [1.0.1] - 2026-07-26

### Fixed

- Synchronized workspace version 1.0.1 lockfile, skill metadata, and changelog.

## [1.0.0] - 2026-07-26

### Security

- Pdfium discovery no longer considers the current working directory. Managed native libraries
  are downloaded with pinned archive/library SHA-256 values and published atomically only after
  bounded extraction and verification.
- Attachment upload credentials are restricted to the Zotero API origin; external upload targets
  receive neither the API key nor Zotero protocol headers. Workspace paths, attachment download
  filenames, graph URLs, annotation geometry, and connector import targets now fail closed at
  their trust boundaries.
- Configuration secrets use redacted diagnostics and atomic restrictive persistence. Remote HTTP
  handling now bounds retries/error bodies and validates OA PDF redirects, addresses, MIME, size,
  and file magic.

### Reliability

- Local Zotero reads use the SQLite Backup API for consistent WAL-aware snapshots. PDF text cache
  sidecars use WAL, busy timeouts, schema versions, and content SHA-256 cache identities.
- JSON failures now use one versioned envelope across runtime, parse, and protocol errors. Batch tag
  writes require explicit confirmation, enforce an affected-item ceiling, and retain per-operation
  partial-state evidence.
- Search and statistics exclude trashed items by default; collection-name ambiguity, note-tag N+1,
  duplicate detection, graph edge budgets, heavy local I/O scheduling, retry semantics, and
  attachment orphan cleanup are hardened for larger libraries.

### Changed

- Effective profile, output format/limit, doctor capability fields, and config initialization now
  report or reject the settings actually used instead of accepting silent no-ops.
- Some legacy 0.x result semantics and error shapes are intentionally tightened. Agent consumers
  should rely on stable error codes and `meta.api_version = 1`, not human-readable messages.

### Engineering

- The five crates inherit workspace lints. Local and GitHub gates use `Cargo.lock`, preserve Rust
  1.85 compatibility, verify version/skill drift, and run on Linux, Windows, and macOS.
- CI adds dependency advisory/license/source checks and unused-dependency analysis. The unused
  `rmcp` workspace declaration is removed until MCP is implemented.

### Migration

- **Rotate the Zotero API key if any release before 1.0.0 was used to upload attachments.** Earlier
  versions could forward that key to an authorization-provided upload host.
- Review automation that depends on 0.x JSON error text, trashed-item inclusion, ambiguous
  collection names, or implicit attachment overwrite. Use the new explicit flags and stable codes.

## [0.6.0] - 2026-07-18

### Added

- **Knowledge graph**: `zot graph build` constructs a local co-authorship,
  citation, and tag-similarity graph from the local SQLite library;
  `zot graph serve` starts a local web UI for interactive graph exploration.
  (`7ab712a`, `d4caef1`)
- **`zot-brainstorm` skill**: new agent skill that turns Zotero references into
  traceable research-brainstorming reports (gap analysis, innovation directions,
  limitation analysis). (`f1a34a1`)
- **`zot-desktop` crate**: a new workspace crate encapsulating connector-based
  local import paths, replacing the removed `zot-bridge` plugin.
- **`CommandOutput` module**: uniform command output layer migrated across all
  7 command groups (graph → annotation/tag/scite/doctor/sync → note/config/write
  → collection/read → library → workspace/mcp). Handlers now return typed
  `CommandOutput` values instead of ad-hoc printing. (`ddbc0b2`–`73b87c9`)
- **pure plan functions**: write/sync/scite command handlers now follow the
  merge.rs pattern of extracting pure plan functions, separating business logic
  from CLI concerns. (`98e7b73`)
- **`require_*` error constructors**: repetitive error construction across the
  CLI layer is replaced by named constructors (`require_item`,
  `require_collection`, etc.). (`d52c55b`)
- **`CliEnvelope::err()` constructor**: unified error envelope creation
  replacing the `ErrorPayload`/`EnvelopeError` dualism. (`bb35df6`)
- **Connector local import**: `zot import <file>` accepts BibTeX/RIS via
  Zotero Desktop's built-in connector. (`3bfee8e`)

### Changed

- `item merge`, `library duplicates-merge`, and `library dedupe` now use the
  Zotero Web API exclusively. Configure `library_id` and `api_key` before
  confirming these operations.
- Zotero Desktop's built-in connector is now the only local write path and is
  limited to importing new BibTeX/RIS records into the selected writable target.
- **Narrow trait decomposition**: five data-domain traits
  (`ItemData`, `CollectionData`, `NoteData`, `TagData`, `SearchData`) extracted
  in `zot-local`; `LocalLibrary` becomes a thin delegator. (`a05e48a`)
- **`rag_engine` orchestrator**: unified indexing logic extracted from the
  dual RAG facade, centralising embedding dispatch and index lifecycle.
  (`7dce14f`)
- **Shared transport layer**: six remote clients (Zotero API, CrossRef, etc.)
  converge on a shared `http.rs` transport with consistent error handling and
  testability. (`9d930a3`)
- **`PdfBackend` replaceability**: `AppContext` accepts a trait-object
  `PdfBackend` for test substitution; construction is centralized in the
  composition root. (`1d11647`)
- **Config consolidation**: four separate config-setting functions replaced by
  a single `apply_setting` path. (`cafe241`)
- JSON payloads no longer include `write_backend`,
  `selected_write_backend`, or `capabilities.desktop_write`. This breaking
  schema change keeps `meta.api_version = 1`; consumers must follow the
  changelog for field additions and removals within the 0.x release line.
- **Skill renamed**: `zotero` agent skill renamed to `zot` to match the CLI
  binary name. (`3d15e02`)

### Fixed

- **PDF download timeout**: `pdf.rs` aligns its download URL with the shared
  network constants and adds the missing request timeout. (`91aa183`)
- **Related-score unification**: the related-item scorer is consolidated to a
  single `graph.rs` implementation; fixes a SQL placeholder bug (`?` vs `$1`)
  that caused incorrect related-item results. (`14a2850`)

### Removed

- Removed the bundled `zot-bridge` XPI, `zot bridge ...`, the global
  `--write-backend` flag, and `zot config set write-backend`.

### Testing

- Semantic-search dimension-validation regression tests aligned to spec.
  (`1052918`)
- Collection/note fake-data tests exercising the `require_*` generic
  helpers. (`c0adb62`)
- Remote-layer `tiny_http` fake adapter; all base URLs are now overridable
  in tests. (`8af6f8b`)

### Documentation

- `docs/agents/skills/zot/SKILL.md` updated for connector-based local import
  workflows. (`8d3affa`)
- `docs/spec/graph.md` records the responsibility boundary between `graph.rs`
  and `db.rs`. (`a0a11ee`)
- `.trellis/spec/` populated with project development guidelines. (`7feeb3b`)
- `skills/zot-brainstorm/SKILL.md` usage entry added. (`a5c8007`)

### Build

- `just` commands adapted for PowerShell execution on Windows. (`f60a8f1`)
- Rust source formatting unified across the workspace. (`2c07623`)
- Workspace version bumped to `0.6.0`. (`dd36072`)

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
