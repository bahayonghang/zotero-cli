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

## Local library search and trash policy

- `zot library search`, `list`, and `stats` exclude Zotero `deletedItems` by
  default. Their explicit `--include-trashed` flag restores the legacy broad
  view; JSON envelopes report the applied choice as
  `meta.trash_policy = "excluded" | "included"`.
- Search computes `total` with SQL and applies deterministic SQL
  `ORDER BY/LIMIT/OFFSET` before hydrating item fields, creators, tags, and
  collections. Memory use therefore follows the requested page size rather
  than the full number of matches.
- Collection arguments resolve an exact key first. A non-key display name is
  accepted only when unique; duplicate names return `collection-ambiguous`
  with sorted candidate keys.

## Duplicate and graph candidate budgets

- `zot library duplicates` and `zot library dedupe` default to
  `--candidate-budget 250000`. This bounds title-similarity pair comparisons;
  it does not cap scanned items. Read-only duplicate results expose
  `scanned_count`, `candidate_pair_count`, `skipped_oversize_blocks`,
  `candidate_budget`, and `truncated`.
- A truncated `library dedupe` run fails with `duplicate-scan-truncated` before
  any Web writer is constructed. Increase the budget and rerun; never apply a
  partial duplicate scan.
- `zot graph` and `zot graph serve` default to `--edge-budget 100000` unique
  candidate pairs. Graph JSON reports the budget, admitted pair count,
  skipped oversize groups, and truncation under `build`. Existing admitted
  pairs may still accumulate later relation signals after the budget fills.
- Zero candidate or edge budgets are invalid. These ceilings bound candidate
  work, not output-node count; graph still includes every node in scope.

## Async local database boundary

- Heavy library search/list/stats, duplicate/dedupe planning, graph builds,
  annotation reads, and workspace membership/import/search queries run through
  the CLI `run_local` blocking boundary. The snapshot open and rusqlite query
  execute together on a blocking worker; remote HTTP writes remain async and
  outside that closure.

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

## Local attachment and PDF sidecar boundaries

- `zot item download` treats an attachment metadata filename as an untrusted
  basename. It rejects traversal/separators and does not overwrite an existing
  destination unless `--force` is explicit.
- Area annotation coordinates must be finite and remain inside the normalized
  unit page: `0 <= x,y < 1`, positive width/height, and endpoints at most 1.
- PDF text caches use WAL, a 5-second SQLite busy timeout, schema version 1,
  and streamed SHA-256 content fingerprints. Workspace indexing keeps the
  existing shared `.md_cache.sqlite` path.
- `graph serve` renders graph fields with DOM APIs, permits clickable item URLs
  only for HTTP(S), and applies CSP plus `nosniff` to every route.

## CrossRef polite-pool contact email

- CrossRef and Unpaywall requests both include a `mailto:` contact in the
  `User-Agent` / query-string. Set `ZOT_CONTACT_EMAIL` to your real address
  to enter the CrossRef polite pool. The default is `noreply@zot.local`,
  which is recognised as an opaque placeholder rather than a real mailbox.

## Remote HTTP and attachment limits

- Eligible GET and `Zotero-Write-Token` requests make at most **3 attempts**.
  `Retry-After` and fallback backoff are capped at **5 seconds** per retry.
  Other mutations are one-shot.
- Remote non-success bodies retain at most **4 KiB** after control-character
  removal and whitespace normalization; larger/unfinished bodies are marked
  `[truncated]`.
- OA automatic PDF downloads allow at most **5 redirects**, validate every
  destination and DNS result, and accept at most **100 MiB** by both declared
  and streamed size. They also require `application/pdf` and `%PDF-` magic.
- Local attachment uploads accept regular files up to **100 MiB**. The limit is
  checked before attachment-item creation; later authorize/upload/register
  failures trigger best-effort orphan cleanup.

## JSON envelope contract

- Executed one-shot `zot --json ...` commands always return exactly one standard envelope from
  `zot-core::CliEnvelope`, including failures from generic runtime errors and CLI parsing.
  Success and error payloads carry `meta.api_version == 1`; `meta.profile == "<active>"` is
  included when a profile is known. Stable generic codes are `runtime-error`,
  `json-serialization`, and `cli-parse`.
- `graph serve` is a long-running human protocol and `completions` emits a raw shell script;
  both reject `--json` with `json-protocol-unsupported`. Clap help/version remain native
  documentation output rather than command envelopes.
- `api_version == 1` identifies the
  envelope family, but individual command payload fields may be added or
  removed during the 0.x release line. Consumers must also follow
  `CHANGELOG.md`; if `api_version` is absent or larger, expect broader schema
  drift.
