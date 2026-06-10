# zot-remote Backend Guidelines

`zot-remote` owns network clients: Zotero Web API writes, attachment upload,
Better BibTeX lookup, open-access metadata/PDF resolution, Semantic Scholar,
Scite, and embedding service calls. It should not parse CLI flags or inspect
the local Zotero SQLite database.

## Pre-Development Checklist

- Read [Directory Structure](./directory-structure.md) before adding a remote
  client or changing module boundaries.
- Read [Database Guidelines](./database-guidelines.md) before adding any
  persistence; this crate currently has none.
- Read [Error Handling](./error-handling.md) before mapping HTTP or JSON
  failures.
- Read [Quality Guidelines](./quality-guidelines.md) before changing batching,
  API preconditions, or normalization behavior.
- Read [Logging Guidelines](./logging-guidelines.md) before adding diagnostics
  around remote calls or secrets.

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | Remote client modules and public exports | Complete |
| [Database Guidelines](./database-guidelines.md) | No-persistence boundary for remote clients | Complete |
| [Error Handling](./error-handling.md) | `ZotError::Remote` and HTTP response handling | Complete |
| [Quality Guidelines](./quality-guidelines.md) | Batching, preconditions, and network-test patterns | Complete |
| [Logging Guidelines](./logging-guidelines.md) | Output-free clients and secret-safe diagnostics | Complete |

## Source References

- `src/zot-remote/src/http.rs`
- `src/zot-remote/src/zotero.rs`
- `src/zot-remote/src/embedding.rs`
- `src/zot-remote/src/oa.rs`
- `src/zot-remote/src/scite.rs`
- `src/zot-remote/src/semantic_scholar.rs`
- `docs/agents/limits.md`
