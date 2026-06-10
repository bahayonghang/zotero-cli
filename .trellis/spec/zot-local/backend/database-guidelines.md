# Database Guidelines

`zot-local` uses `rusqlite` directly. There are two different database
categories with different rules:

- `zotero.sqlite` is Zotero-owned and must be opened read-only.
- `zot` sidecar SQLite files are owned by this CLI and may be created,
  migrated, and written.

## Zotero SQLite Reads

- Open Zotero's main database through `LocalLibrary::open`, which checks for
  `zotero.sqlite`, opens a read-only immutable URI, and falls back to a
  temporary read-only copy with `sqlite-wal` and `sqlite-shm` copied when
  needed.
- Use `prepare_cached`, `params!`, and `params_from_iter` instead of string
  interpolation for values.
- Escape user-provided `LIKE` terms with `escape_like` and pair the SQL with
  `ESCAPE '\'`. This applies to general search, notes, annotations, creators,
  and tags.
- Return `Ok(None)` or `Ok(Vec::new())` for expected misses, as
  `get_item` and `get_notes` do.

## Sidecar Databases

- `RagIndex::open` creates and owns the `chunks`, `bm25_terms`, and
  `index_meta` tables for semantic/RAG search.
- Sidecar indexes set `PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;`.
- `PdfCache` owns a simple `cache(cache_key, content)` table for extracted PDF
  text.
- Bulk writes should run through `RagIndex::with_write_tx` to amortize fsync
  cost and keep chunks/terms/embeddings consistent.

## Migration Pattern

Sidecar migrations are in code, not an external migration tool. The current
example is `RagIndex::migrate_embedding_to_blob`, which detects a legacy TEXT
`embedding` column via `PRAGMA table_info(chunks)`, converts JSON vectors to
little-endian `f32` BLOBs, and drops the legacy column.

`src/zot-local/tests/semantic_index.rs` verifies:

- BLOB embeddings survive reopen.
- Legacy TEXT embeddings migrate on open.
- Bulk writes commit as one transaction.
- BM25 average document length cache invalidates after corpus mutations.

## Code Example

Use generated `?` markers for variable-size `IN` queries:

```rust
std::iter::repeat_n("?", count).collect::<Vec<_>>().join(",")
```

Pair this with `params_from_iter`, as `db.rs` and `workspace.rs` do.

## Avoid

- Never write to Zotero's `zotero.sqlite` directly.
- Do not let `%`, `_`, or `\` behave as wildcards for user search text.
- Do not call `RagIndex::open` from status paths when the desired behavior is
  "report missing index without creating it"; use `SemanticStore::status_at`.
- Do not truncate large inputs silently. `docs/agents/limits.md` documents
  current semantic search and batching ceilings.
