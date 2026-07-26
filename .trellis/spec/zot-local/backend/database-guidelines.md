# Database Guidelines

`zot-local` uses `rusqlite` directly. There are two different database
categories with different rules:

- `zotero.sqlite` is Zotero-owned and must be opened read-only.
- `zot` sidecar SQLite files are owned by this CLI and may be created,
  migrated, and written.

## Zotero SQLite Reads

- Open Zotero's main database through `LocalLibrary::open`, which checks for
  `zotero.sqlite` and creates a transactionally consistent temporary snapshot
  through SQLite's Backup API. The source is always opened read-only.
- Use `prepare_cached`, `params!`, and `params_from_iter` instead of string
  interpolation for values.
- Escape user-provided `LIKE` terms with `escape_like` and pair the SQL with
  `ESCAPE '\'`. This applies to general search, notes, annotations, creators,
  and tags.
- Return `Ok(None)` or `Ok(Vec::new())` for expected misses, as
  `get_item` and `get_notes` do.
- Trash filtering on primary-item search is the default:
  `SearchOptions::default().exclude_trashed == true`. Callers may explicitly set
  it to `false` only for a surface that advertises trash inclusion. Notes,
  annotations, duplicates, graph, collection, and workspace reads stay
  non-trash by default.

## Scenario: Consistent Zotero database snapshot

### 1. Scope / Trigger

This contract applies whenever `LocalLibrary::open`, the source connection,
SQLite backup policy, snapshot metadata, or doctor local-SQLite diagnostics
change. It prevents live WAL state from being ignored and prevents a manually
copied DB/WAL/SHM set from being treated as a consistent source of write
decisions (QW-05/M-01, task `07-26-fix-sqlite-snapshot`).

### 2. Signatures

- `LocalLibrary::open(data_dir, scope) -> ZotResult<LocalLibrary>` owns the
  source-to-snapshot lifecycle.
- `LocalLibrary::snapshot_meta() -> &LibrarySnapshotMeta` exposes immutable
  source mtime, snapshot UTC time, and `userdata` schema version.
- Private `connect_with_policy(db_path, policy)` exists only to inject bounded
  timing in tests; production callers use the fixed default policy.

### 3. Contracts

- Open the source path with `SQLITE_OPEN_READ_ONLY` and a five-second busy
  timeout. Never use a URI `immutable=1` flag, write/checkpoint the source, or
  copy `zotero.sqlite`, `-wal`, and `-shm` independently.
- Create a destination in a task-owned `TempDir`; use `backup::Backup::step`
  with fixed page batches and a short yield. A continuous `Busy`/`Locked` run
  has a five-second monotonic retry limit.
- Do not expose the destination before `StepResult::Done`. Drop its writable
  connection, reopen it with `SQLITE_OPEN_READ_ONLY`, and require
  `PRAGMA quick_check` to return `ok`.
- `LocalLibrary` owns the `TempDir` for its full lifetime. `db_path()` continues
  to return the Zotero source path; the ephemeral snapshot path stays private.
- Metadata timestamps are UTC RFC 3339. An unavailable source mtime is `null`,
  never a fabricated value.

### 4. Validation & Error Matrix

- Missing source -> existing `db-not-found` before snapshot creation.
- Busy/locked source, backup init/step, or validation -> `zotero-db-busy` with
  a close-Zotero-or-retry hint; no fallback copy.
- Other source open failure -> `open-zotero-db`.
- Other backup failure -> `snapshot-zotero-db`.
- Snapshot read-only reopen failure -> `open-zotero-snapshot`.
- `quick_check` error or non-`ok` result ->
  `zotero-db-snapshot-integrity`; the temporary destination is discarded.

### 5. Good / Base / Bad Cases

- Good: Zotero commits data into WAL while reads continue; Backup API returns a
  snapshot whose cross-table invariants reflect one transaction boundary.
- Base: a closed Zotero database snapshots once, reports metadata, and serves
  all `LocalLibrary` reads from the read-only temporary connection.
- Bad: pass `immutable=1` for a live database, use `fs::copy` on DB/WAL/SHM,
  retry a permanent lock forever, or return a writable/unchecked destination.

### 6. Tests Required

- Prove committed, uncheckpointed WAL data is visible and snapshot creation
  neither truncates nor checkpoints the source WAL.
- Hold an exclusive source lock with a short injected policy and assert the
  exact `zotero-db-busy` payload and actionable hint.
- Repeatedly snapshot under a concurrent writer; assert cross-table invariants
  and `quick_check='ok'` on every result.
- Assert metadata serialization in doctor, run `cargo test -p zot-local`,
  focused doctor tests, workspace clippy, and `just ci`.

### 7. Wrong vs Correct

```rust
// Wrong: both paths bypass SQLite's consistency protocol.
Connection::open("file:zotero.sqlite?mode=ro&immutable=1")?;
fs::copy("zotero.sqlite-wal", "snapshot.sqlite-wal")?;

// Correct: SQLite owns WAL visibility and the transaction boundary.
let source = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
let backup = backup::Backup::new(&source, &mut temporary_destination)?;
loop {
    match backup.step(PAGES_PER_STEP)? {
        StepResult::Done => break,
        StepResult::More => yield_to_writer(),
        StepResult::Busy | StepResult::Locked => retry_before_deadline()?,
        _ => return Err(unsupported_backup_state()),
    }
}
```

## Scenario: Bounded local search, duplicate, and graph queries

### 1. Scope / Trigger

Apply this contract when changing primary-item search, collection resolution,
note hydration, duplicate detection, graph construction, or their public result
models. It prevents trash leakage, nondeterministic collection selection,
whole-result hydration, and unbounded pair expansion.

### 2. Signatures

```rust
LocalLibrary::search(SearchOptions) -> ZotResult<SearchResult>
LocalLibrary::find_duplicates(method, collection, group_limit, candidate_budget)
    -> ZotResult<DuplicateScanResult>
LocalLibrary::build_knowledge_graph(&GraphOptions) -> ZotResult<KnowledgeGraph>
```

`GraphOptions.edge_budget` defaults to `100_000`; duplicate candidate budget
defaults are owned by the CLI and passed explicitly.

### 3. Contracts

- Search builds one bound predicate set, runs a separate `COUNT(*)`, selects
  page IDs with deterministic `ORDER BY ... , i.key`, then hydrates only those
  IDs through `get_items_batch`.
- Query field/creator/tag/fulltext branches are OR; collection/type/tag/creator/
  year filters are AND. User `LIKE` values remain escaped and bound.
- Collection lookup checks exact key first. A name resolves only when exactly
  one key matches; multiple names fail with sorted candidate keys.
- `get_notes` collects IDs first and calls `load_item_tags_batch`; never restore
  per-note tag queries.
- Duplicate DOI groups are exact and title comparisons are admitted through
  deterministic blocks. `group_limit` truncates output groups only; it must not
  truncate scanned input. Every result reports scan, pair, budget, skipped-block,
  threshold, and truncation metadata.
- Graph budget limits only new unique pairs. An admitted pair may still receive
  later relation signals; oversize groups are skipped and counted.

### 4. Validation & Error Matrix

| Condition | Result |
| --- | --- |
| duplicate candidate budget is zero | `duplicate-candidate-budget` |
| graph edge budget is zero | `graph-edge-budget` |
| collection key/name missing | `collection-not-found` |
| collection name matches multiple keys | `collection-ambiguous`, sorted keys in hint |
| duplicate pair budget exhausted | partial groups plus `truncated=true` |
| graph pair budget exhausted | bounded graph plus `build.truncated=true` |

### 5. Good / Base / Bad Cases

- Good: a 50-item page hydrates 50 IDs while `total` comes from SQL count.
- Base: an untruncated small duplicate/graph fixture keeps stable groups,
  nodes, edges, metrics, and key ordering.
- Bad: load 10,000 full items before pagination, use `key=? OR name=? LIMIT 1`,
  silently stop duplicate input at 10,000, or expand every graph clique.

### 6. Tests Required

- Fixture tests cover trash inclusion/exclusion, OR/AND and literal-LIKE
  semantics, all sort fields, offset/limit, key-first collection lookup, and
  deterministic ambiguity hints.
- Duplicate tests assert DOI/title groups, scan metadata, exact budget
  saturation, fail-visible truncation, and a 10k synthetic bound.
- Graph tests assert admitted-pair updates, edge budget, oversize counts,
  unchanged small-fixture metrics, and a 50k synthetic no-clique bound.
- Run `cargo test -p zot-local` and `just ci`.

### 7. Wrong vs Correct

```rust
// Wrong: hydrate and sort the full candidate set in Rust.
let all = get_items_batch(&candidate_ids)?;
all.sort_by(...);
let page = all.into_iter().skip(offset).take(limit).collect();

// Correct: count and page in SQL, then hydrate page IDs only.
let total = count_matching(&predicates, &params)?;
let page_ids = select_page(&predicates, &params, sort, limit, offset)?;
let page = get_items_batch(&page_ids)?;
```

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

When one query mixes a generated `IN (?,...)` list with a fixed parameter,
put the fixed parameter first as `?1` and the `IN` list after it. SQLite
numbers a plain `?` as one-greater-than-the-largest index seen so far, so an
`IN (?,?)`-first clause followed by `?1` makes `?1` alias the first `IN`
slot; the statement then expects one fewer bind than supplied and rusqlite
fails with `InvalidParameterCount`. This was a live bug in
`get_related_items` (any item with collections or tags errored) until
2026-07-07; `count_shared_ids` in `db.rs` shows the correct ordering — the
fixed `?1` is bound first and the generated `IN (...)` list comes last.

## Avoid

- Never write to Zotero's `zotero.sqlite` directly.
- Never restore `immutable=1` or filesystem DB/WAL/SHM snapshot copying.
- Do not let `%`, `_`, or `\` behave as wildcards for user search text.
- Do not place a numbered `?N` after a generated plain-`?` `IN` list in the
  same statement; the indexes collide (see Code Example above).
- Do not call `RagIndex::open` from status paths when the desired behavior is
  "report missing index without creating it"; use `SemanticStore::status_at`.
- Do not truncate large inputs silently. `docs/agents/limits.md` documents
  current semantic search and batching ceilings.
