# Directory Structure

`zot-local` is the local data layer. Keep local Zotero reads, local sidecar
indexes, PDF extraction, and workspace storage here. Do not put clap handlers
or remote writes in this crate.

## Directory Layout

```text
src/zot-local/src/
├── citation.rs       # Citation formatting and export helpers
├── db.rs             # LocalLibrary over zotero.sqlite
├── graph.rs          # Knowledge-graph assembly + analysis (pure, no SQLite)
├── lib.rs            # Public exports
├── pdf.rs            # PdfBackend, PdfiumBackend, PdfCache
├── rag_engine.rs     # Shared reindex/embedding engine behind both RAG facades (private)
├── semantic.rs       # Library-level semantic index facade
├── workspace.rs      # WorkspaceStore and RagIndex primitives
└── workspace_rag.rs  # Workspace-specific RAG facade
```

Tests are split between inline module tests and integration tests:

```text
src/zot-local/tests/
├── search_regression.rs
└── semantic_index.rs
```

## Module Ownership

- `db.rs` owns `LocalLibrary`, `SearchOptions`, Zotero schema joins, collection
  tree reads, child item reads, duplicate grouping, note/annotation reads, and
  fixture-backed local search behavior.
- `graph.rs` owns knowledge-graph assembly (`assemble_graph`) and pure
  structural analysis (degree, union-find components, deterministic label
  propagation). It never opens SQLite. `LocalLibrary::build_knowledge_graph`
  lives in `db.rs` and performs the reads — reusing `search`/`get_items_batch`
  plus one `itemRelations` query — then hands loaded `Item`s to `graph.rs`,
  because `LocalLibrary`'s private fields are only reachable from `db.rs`. Keep
  DB access in `db.rs`; keep deterministic computation in `graph.rs`.
- Pair relatedness has exactly one weight table: `graph.rs::score_pair` over
  `PairAccum` signals (explicit relation 100, coauthor 8 per shared author,
  tag 5 per shared tag, collection 1 per shared collection). Both
  `assemble_graph` edge weights and `LocalLibrary::get_related_items` delegate
  to it; `get_related_items` only fetches raw signals (explicit relation
  pairs plus shared creator/tag/collection counts via `count_shared_ids`)
  with no weights or thresholds in SQL. Signal fetch is scoped to primary
  items — child `attachment`/`note`/`annotation` rows are excluded in
  `count_shared_ids`, matching the graph's node universe, so unprintable
  items never consume `limit` slots (candidate scoping, not a score rule).
  Decisions recorded 2026-07-07
  (task 07-07-related-scorer): the coauthor signal now counts toward
  `zot related` (it was missing), and the old `HAVING cnt >= 2` tag threshold
  is gone — a single shared tag scores 5. `GraphOptions.min_shared_tags`
  stays a graph-only edge-emission gate applied before scoring, never a
  weight rule; future noise filtering belongs in the scorer as an explicit
  parameter, not in fetch SQL.
- `pdf.rs` owns the `PdfBackend` trait, Pdfium loading/auto-download probing,
  text extraction, outline extraction, annotation geometry, and the PDF text
  cache database. Its Pdfium bootstrap downloader keeps its own
  `reqwest::blocking` stack — a deliberate boundary decision (2026-07-08,
  task 07-07-pdf-http): zot-local must not depend on zot-remote, the download
  is a one-shot first-run bootstrap, and blocking is appropriate in this sync
  crate. Timeout and User-Agent come from `zot_core::net`
  (`CONNECT_TIMEOUT`/`REQUEST_TIMEOUT`/`USER_AGENT`), shared with zot-remote's
  `HttpRuntime`, so the two stacks cannot drift. Do not re-raise unifying the
  stacks; do not hardcode timeouts or UA strings here.
- `workspace.rs` owns durable workspace TOML files and the generic `RagIndex`
  schema used by both library and workspace search.
- `rag_engine.rs` owns the single indexing orchestration shared by both RAG
  facades (decision 2026-07-07, task 07-07-rag-engine): the reindex loop
  (item walk, PDF-text cache use, chunking, pending-embedding collection,
  parameterized by a prune predicate and `RefreshPolicy`), embedding
  writeback with batch dimension validation and the `embedding.dim` meta,
  query-time `validate_query_embedding`, the shared
  `PendingEmbedding`/`ReindexStats` types, the chunking constants, and the
  `RagLibrary` trait plus its `LocalLibrary` impl. The module is private;
  `lib.rs` re-exports only the types facade callers need. Do not re-grow
  per-facade copies of this loop or of dimension tracking.
- `semantic.rs` is the thin library-level facade. `SemanticStore` holds the
  index path and optional `PdfCache`, delegates reindex/writeback/query
  validation to `rag_engine` (pruning keys no longer present in the
  library), and leaves async embedding calls to the caller.
- `workspace_rag.rs` is the thin workspace facade. It owns the
  `<workspace>.idx.sqlite` and `.md_cache.sqlite` sidecar paths and
  delegates to `rag_engine`, pruning keys that left the workspace and
  skipping already-indexed ones.

## Naming Conventions

- Public local-library entry points are methods on `LocalLibrary`.
- Sidecar index operations are methods on `RagIndex`, `SemanticStore`, or
  `WorkspaceRagStore`.
- Search and indexing knobs use small typed structs/enums, such as
  `SearchOptions`, `SortField`, `SortDirection`, `HybridMode`,
  `ReindexOpts`, and `WorkspaceReindexOpts`.

## Code Example

`WorkspaceRagStore::open` demonstrates the workspace sidecar layout used by
the CLI and docs:

```rust
let index_path = store.root().join(format!("{workspace_name}.idx.sqlite"));
let md_cache_path = store.root().join(".md_cache.sqlite");
```

Keep new workspace-local storage next to the workspace root, not in the
Zotero data directory.

## Avoid

- Do not add Web API writes here; use `zot-remote`.
- Do not parse CLI flags here; use `zot-cli` and pass typed options in.
- Do not depend on the legacy `ref/` Python implementation when reasoning
  about active behavior.
