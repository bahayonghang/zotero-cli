# zot-local Backend Guidelines

`zot-local` owns local Zotero reads, PDF extraction/cache behavior, citation
formatting, and local workspace/RAG indexes. Treat the user's
`zotero.sqlite` as read-only source data; writable SQLite databases in this
crate are sidecars owned by `zot`.

## Pre-Development Checklist

- Read [Directory Structure](./directory-structure.md) before adding local
  library, PDF, semantic, or workspace behavior.
- Read [Database Guidelines](./database-guidelines.md) before writing any SQL,
  sidecar schema, or query filter.
- Read [Error Handling](./error-handling.md) before mapping `rusqlite`,
  filesystem, or Pdfium failures.
- Read [Quality Guidelines](./quality-guidelines.md) before changing search,
  indexing, chunking, or tests.
- Read [Logging Guidelines](./logging-guidelines.md) before adding diagnostics;
  this crate should return data/errors rather than print.

## Guidelines Index

| Guide                                           | Description                                                       | Status   |
| ----------------------------------------------- | ----------------------------------------------------------------- | -------- |
| [Directory Structure](./directory-structure.md) | Local-library, PDF, citation, semantic, and workspace ownership   | Complete |
| [Database Guidelines](./database-guidelines.md) | Read-only Zotero SQLite access and sidecar schemas                | Complete |
| [Error Handling](./error-handling.md)           | `ZotError::Database`, `ZotError::Pdf`, and `ZotError::Io` mapping | Complete |
| [Quality Guidelines](./quality-guidelines.md)   | Search/indexing invariants and tests                              | Complete |
| [Logging Guidelines](./logging-guidelines.md)   | Output-free local library code                                    | Complete |

## Source References

- `src/zot-local/src/db.rs`
- `src/zot-local/src/pdf.rs`
- `src/zot-local/src/rag_engine.rs`
- `src/zot-local/src/semantic.rs`
- `src/zot-local/src/workspace.rs`
- `src/zot-local/src/workspace_rag.rs`
- `src/zot-local/tests/search_regression.rs`
- `src/zot-local/tests/semantic_index.rs`
- `docs/agents/limits.md`
