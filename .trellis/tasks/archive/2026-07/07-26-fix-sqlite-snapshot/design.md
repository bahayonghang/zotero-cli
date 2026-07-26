# Design: SQLite consistent snapshot

## Architecture and boundary

`LocalLibrary::open` remains the public facade. Its private connection path becomes:

```text
source zotero.sqlite
  -> open READ_ONLY + busy_timeout(5s)
  -> SQLite Backup API, paged, bounded busy retry
  -> task-owned TempDir/zotero.sqlite
  -> close writable destination
  -> reopen snapshot READ_ONLY
  -> PRAGMA quick_check + userdata schema query
  -> LocalLibrary { conn, _temp_dir, snapshot_meta }
```

The source connection participates in SQLite locking and WAL semantics. No source URI,
`immutable=1`, filesystem copy, checkpoint, or source write is allowed.

## Contracts

### Snapshot policy

- Production busy timeout/deadline: 5 seconds.
- Backup uses a fixed positive page batch and a short pause between incomplete steps.
- `Busy` and `Locked` may retry only until the monotonic deadline; all SQLite errors pass
  through one mapper so busy/locked always become `zotero-db-busy`.
- Tests inject a shorter private policy to prove bounded failure without slowing the suite.

### Integrity and lifecycle

- Destination is private until `Backup::step` returns `Done`.
- The writable destination and backup handle are dropped before the snapshot is reopened
  with `SQLITE_OPEN_READ_ONLY`.
- `PRAGMA quick_check` must return exactly `ok` before `LocalLibrary` construction.
- `TempDir` is stored on `LocalLibrary`; every early error drops it and its partial database.

### Metadata

```rust
pub struct LibrarySnapshotMeta {
    pub source_modified_at: Option<String>,
    pub snapshot_created_at: String,
    pub schema_version: Option<i64>,
}
```

Times use UTC RFC 3339. The source path remains available through `db_path()`; the ephemeral
snapshot path stays private. Doctor serializes this struct under
`capabilities.local_sqlite_read.snapshot` and reuses `schema_version` for its existing field.

## Error matrix

| Condition | Code | Behavior |
|---|---|---|
| source missing | `db-not-found` | unchanged, before snapshot work |
| SQLite busy/locked | `zotero-db-busy` | bounded retry then actionable failure |
| source open failure | `open-zotero-db` | no fallback copy |
| backup init/step failure | `snapshot-zotero-db` | temp destination removed |
| snapshot reopen failure | `open-zotero-snapshot` | temp destination removed |
| quick_check non-ok | `zotero-db-snapshot-integrity` | snapshot rejected |
| source mtime unavailable | no error | metadata field is `null` |

## Compatibility and trade-offs

- Each `LocalLibrary::open` now pays snapshot I/O and temporary disk cost. This is accepted for
  correctness; cross-command caching would require lifecycle and staleness policy outside scope.
- `db_path()` continues to identify the user's source DB, preventing callers from retaining an
  ephemeral path.
- Existing read methods and library-scope resolution are unchanged because they receive the same
  `Connection` type after the new boundary.

## Rollback

Revert the task commits as a unit. Do not restore `immutable=1` or manual sidecar copying as a
partial rollback; the operational fallback is to close Zotero and retry a normal read-only open.
