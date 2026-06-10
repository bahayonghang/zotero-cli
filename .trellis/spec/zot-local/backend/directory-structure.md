# Directory Structure

`zot-local` is the local data layer. Keep local Zotero reads, local sidecar
indexes, PDF extraction, and workspace storage here. Do not put clap handlers
or remote writes in this crate.

## Directory Layout

```text
src/zot-local/src/
├── citation.rs       # Citation formatting and export helpers
├── db.rs             # LocalLibrary over zotero.sqlite
├── lib.rs            # Public exports
├── pdf.rs            # PdfBackend, PdfiumBackend, PdfCache
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
- `pdf.rs` owns the `PdfBackend` trait, Pdfium loading/auto-download probing,
  text extraction, outline extraction, annotation geometry, and the PDF text
  cache database.
- `workspace.rs` owns durable workspace TOML files and the generic `RagIndex`
  schema used by both library and workspace search.
- `semantic.rs` wraps library-wide indexing. It coordinates `LocalLibrary`,
  `RagIndex`, optional `PdfCache`, and pending embeddings but leaves async
  embedding calls to the caller.
- `workspace_rag.rs` wraps workspace-specific indexing and querying, including
  `<workspace>.idx.sqlite` and `.md_cache.sqlite` sidecars.

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
