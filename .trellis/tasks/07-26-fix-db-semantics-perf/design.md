# Design: 本地 DB 语义与查询性能

## Boundaries

- `zot-local::db` owns search SQL, collection resolution, note batching, duplicate candidate projection and graph data loading.
- `zot-local::graph` remains a pure deterministic assembler and owns the candidate-edge budget.
- `zot-core::model` owns additive machine-readable duplicate/graph result types and optional envelope trash policy.
- `zot-cli` owns flags, envelope metadata, human warnings and the Tokio blocking boundary.

## Search Query Plan

Replace the HashSet pipeline with two queries sharing one static-fragment predicate builder:

1. `COUNT(*)` over primary items satisfying library, item type, trash, query OR branches and structured AND filters.
2. Page-ID selection with a correlated scalar sort expression, deterministic key tie-break, `LIMIT` and `OFFSET`.
3. Existing `get_items_batch` hydrates only the selected IDs and preserves their SQL order.

All user values remain bound parameters. Dynamic SQL contains only fixed fragments selected from enums/optional filters. Query text uses `EXISTS` branches for fields, creators, tags and Zotero fulltext rows, preserving the current union semantics without materializing a Rust candidate set.

## Trash Policy

Keep the existing `exclude_trashed` field for source compatibility but flip its `Default` to `true`. CLI `--include-trashed` maps to `exclude_trashed = false`. Stats accepts an explicit include flag and applies the same predicate to every aggregate. `EnvelopeMeta.trash_policy` is optional and omitted outside search/list/stats.

## Duplicate Scan

Load a minimal ordered projection `(item_id, key, title, doi, date, first_creator_surname)` for the full non-trash scope. DOI candidates are exact normalized groups. Title candidates enter deterministic blocks keyed by normalized first 12 characters plus year and normalized first-author surname; empty optional dimensions use stable sentinel values.

A sorted pair set deduplicates candidates shared across blocks. Each new title comparison consumes one unit from `candidate_budget`; on exhaustion, later new pairs are skipped and `truncated=true`. Exact DOI groups do not consume Levenshtein budget. Only item IDs participating in accepted groups are hydrated, then group order and item order are stabilized by keys. `DuplicateScanResult` carries scan count, comparisons, skipped oversize blocks, threshold, budget and truncation.

Oversize title blocks are skipped rather than expanded beyond the remaining budget and increment `skipped_oversize_blocks`. Dedupe rejects any truncated scan before writer construction.

## Graph Budget

`assemble_graph` walks BTreeMap groups in deterministic order. `accumulate` updates an existing pair even when the budget is full, but refuses a new unique pair beyond `edge_budget`. It returns counts through a small internal accumulator. `KnowledgeGraph.build` records candidate pair count, skipped oversize groups, budget and truncation. This bounds pair-map memory while preserving all relation signals for admitted pairs.

Graph node hydration remains whole-scope because the public graph returns every node. The task bounds clique expansion, not node output size.

## Blocking Boundary

`run_local(config, scope, closure)` clones only owned inputs before `spawn_blocking`, opens `LocalLibrary` inside the blocking worker and returns an owned result. It parallels `run_pdf` but uses a database-specific join error. Handlers build `CommandOutput` after awaiting the result so rendering stays on the async task. Dedupe builds its pure plan entirely in the blocking closure and performs remote writes only after return.

## Compatibility And Failure Semantics

- CLI defaults intentionally change to exclude trash; `--include-trashed` is the explicit compatibility escape hatch.
- Search/list success data remains an item array; only optional envelope meta is added.
- `library duplicates` data becomes a structured scan result so truncation cannot be hidden.
- Invalid zero budgets return typed `InvalidInput` before database work.
- Collection ambiguity and truncated dedupe fail before any remote client/writer construction.
- Graph result additions are additive fields; existing nodes/edges/metrics remain stable when not truncated.

## Validation Strategy

- Inline SQLite fixture tests for trash, collection ambiguity, note batching behavior and search golden semantics.
- Pure duplicate/graph tests for deterministic grouping, budget saturation, oversize blocks and 10k/50k synthetic bounds.
- CLI parse/output tests for new flags, trash meta and dedupe fail-closed behavior.
- Current-thread Tokio test proving `run_local` does not block the runtime worker.
- `cargo test -p zot-local`, `cargo test -p zot-cli`, `git diff --check`, then `just ci`.
