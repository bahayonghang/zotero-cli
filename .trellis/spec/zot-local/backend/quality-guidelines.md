# Quality Guidelines

Most `zot-local` regressions are data-shape, query, or sidecar-index
regressions. Add narrow tests around the behavior being changed.

## Required Patterns

- Preserve primary-item filtering. `db.rs` excludes `attachment`, `note`, and
  `annotation` for top-level library searches; `search_regression.rs` verifies
  this for empty search, field search, list, and recent items.
- Preserve literal `LIKE` semantics. `escape_like` has unit tests for `%`, `_`,
  and `\`; new search paths must use the same helper.
- Preserve deterministic ordering after broad set collection. Existing code
  sorts items by explicit sort fields or key and sorts workspaces by name.
- Preserve sidecar index integrity. Use `with_write_tx` for multi-step RAG
  writes, invalidate cached BM25 stats after chunk mutations, and validate
  embedding counts/dimensions before writeback.
- Keep heavy PDF work behind the `PdfBackend` trait so CLI code can offload it
  and tests can use focused fakes.
- Follow the Pdfium native-library discovery contract below. Discovery is a
  security boundary, not a convenience path list.

## Testing Requirements

- Run `cargo test -p zot-local` for local data-layer changes.
- Add fixture-backed tests in `src/zot-local/tests/search_regression.rs` for
  Zotero schema/query behavior.
- Add sidecar-index tests in `src/zot-local/tests/semantic_index.rs` or inline
  `workspace.rs` tests for index schema, migration, and query behavior.
- Add PDF-specific unit tests in `pdf.rs` when changing Pdfium setup, cache
  paths, archive extraction, page range behavior, or annotation geometry.

## Scenario: Pdfium native-library discovery

### 1. Scope / Trigger

This contract applies whenever `pdf.rs` changes Pdfium discovery, probing, or
binding. It prevents a library planted in an untrusted project CWD from being
loaded by `zot doctor` or another Pdfium consumer (P0-01, task
`07-26-fix-pdfium-cwd-rce`).

### 2. Signatures

- `candidate_library_paths(library_name: &Path) -> Vec<PathBuf>` owns ordered
  path discovery.
- `PdfiumBackend::pdfium(mode: PdfiumLoadMode) -> ZotResult<Pdfium>` binds only
  discovered paths, then optionally invokes the managed downloader.

### 3. Contracts

- Candidate order is `ZOT_PDFIUM_LIB_PATH`, `PDFIUM_LIB_PATH`, executable
  adjacent, then managed cache under `ZOT_PDFIUM_CACHE_DIR` or the system cache.
- Env overrides may name a file or directory and are explicit operator opt-in.
- Never add `env::current_dir()` or call `Pdfium::bind_to_system_library()`;
  the latter loads a bare platform name through the platform default search.
- Download integrity and atomic installation belong to download-verify work;
  they must not widen the discovery trust set.

### 4. Validation & Error Matrix

- Candidate missing -> skip it without binding.
- Candidate exists and binds -> return ready Pdfium.
- Candidate exists but fails -> preserve the first typed `ZotError::Pdf`.
- `ProbeOnly` with no usable candidate -> return `pdfium-unavailable`.
- `AllowDownload` on a supported target -> use the managed download path; on
  an unsupported target, return the preserved bind error or manual setup error.

### 5. Good / Base / Bad Cases

- Good: set `ZOT_PDFIUM_LIB_PATH` to a reviewed absolute file or directory.
- Base: use executable-adjacent deployment or the application-managed cache.
- Bad: place Pdfium in the project CWD or load `pdfium.dll` / `libpdfium.*` by
  a bare name and rely on platform search order.

### 6. Tests Required

- Keep `candidate_library_paths_only_uses_trusted_sources` green (or an
  equivalent exact-source regression) without mutating process env or CWD.
- Run `cargo test -p zot-local` and the workspace clippy gate after changing
  discovery or binding.

### 7. Wrong vs Correct

```rust
// Wrong: both forms can reach an untrusted CWD.
paths.push(env::current_dir()?.join(library_name));
Pdfium::bind_to_system_library()?;

// Correct: bind only a path-qualified, policy-approved candidate.
for candidate in candidate_library_paths(library_name) {
    if candidate.exists() {
        return bind_pdfium_from_path(&candidate);
    }
}
```

## Scenario: Verified Pdfium download and installation

### 1. Scope / Trigger

This contract applies whenever the managed Pdfium downloader, release manifest,
archive extraction, cache naming, or cache publication changes. It prevents a
truncated, substituted, wrong-platform, oversized, or concurrently installed
artifact from becoming a trusted native-library candidate (P1-01, task
`07-26-fix-pdfium-download-verify`).

### 2. Signatures

- `download_target_for(os, arch, target_env) -> Option<PdfiumDownloadTarget>`
  selects a pinned archive path plus archive and library SHA-256 values.
- `install_pdfium_library(target, library_name, cache_dir, download)
  -> ZotResult<PathBuf>` owns locking, verification, extraction, and publication.
- `verified_managed_cache_path(cache_dir, target, library_name)
  -> Option<PathBuf>` admits only a hash-matching managed artifact to discovery.

### 3. Contracts

- Every supported target has an in-source archive SHA-256, expected regular-file
  entry path, and extracted-library SHA-256 for the pinned Pdfium version.
- Hold the per-cache `.install.lock`, then recheck the final library before any
  download. Stream the archive into a same-directory temporary file with a
  32 MiB limit; do not buffer an unbounded response in memory.
- Verify the complete archive SHA-256 before opening it. Copy only the exact
  expected regular-file entry, cap the extracted library at 128 MiB, and verify
  its complete SHA-256 before publication.
- Sync temporary files before `NamedTempFile::persist`, use a final filename
  containing the library hash prefix, and sync the cache directory where the
  platform supports it. Temporary files must disappear on every failure.
- Candidate discovery must recompute the full library SHA-256. A legacy bare
  cache name or a hash-prefixed file with tampered content is not trusted.

### 4. Validation & Error Matrix

- Unsupported target or musl Linux -> no managed download target.
- Archive above 32 MiB -> `pdfium-archive-too-large`.
- Archive SHA-256 mismatch, including truncation or wrong archive ->
  `pdfium-archive-checksum`.
- Expected entry missing or not a regular file ->
  `pdfium-archive-missing-library` or `pdfium-archive-entry-type`.
- Library above 128 MiB or SHA-256 mismatch -> `pdfium-library-too-large` or
  `pdfium-library-checksum`.
- Existing final path with matching full hash -> reuse it without downloading;
  any nonmatching path must never be returned as a candidate.

### 5. Good / Base / Bad Cases

- Good: two processes contend on `.install.lock`; the second reuses the verified
  artifact and no second download occurs.
- Base: a clean cache downloads the pinned archive, verifies both hashes, and
  publishes one hash-bound library path.
- Bad: extract the archive wholesale, trust archive entry metadata alone, reuse
  `pdfium.dll` / `libpdfium.*` by name, or publish before checksum verification.

### 6. Tests Required

- Assert all supported target tuples and their exact archive/library hashes.
- Reject tampered, truncated, wrong-platform, missing-entry, symlink-entry,
  library-mismatch, and oversized fixtures without publishing a final file.
- Prove legacy and tampered managed-cache files are excluded, a verified file is
  reused, and concurrent installers invoke the downloader exactly once.
- Run `cargo test -p zot-local`, workspace clippy with `-D warnings`, and `just ci`.

### 7. Wrong vs Correct

```rust
// Wrong: unbounded bytes and unverified direct extraction become executable state.
let bytes = response.bytes()?;
archive.unpack(cache_dir)?;

// Correct: lock, stream with a cap, verify twice, then publish the temp file.
let _lock = lock_cache(cache_dir)?;
let archive = download_bounded_temp(url, MAX_PDFIUM_ARCHIVE_BYTES)?;
verify_sha256(archive.path(), target.archive_sha256)?;
let library = extract_expected_regular_file(&archive, target)?;
verify_sha256(library.path(), target.library_sha256)?;
library.persist(hash_bound_path(target))?;
```

## Code Example

`WorkspaceStore::save` writes through a temporary file and syncs before
persisting, with a Windows replacement path:

```rust
let mut temp = tempfile::NamedTempFile::new_in(&self.root)?;
temp.write_all(raw.as_bytes())?;
temp.as_file_mut().sync_all()?;
#[cfg(target_os = "windows")]
if path.exists() {
    std::fs::remove_file(&path)?;
}
temp.persist(&path)?;
```

Keep this atomic-save style for workspace TOML updates.

## Operational Limits

- `docs/agents/limits.md` documents semantic search as an O(N) scan over
  indexed chunks, with a comfortable ceiling around 10,000 chunks per index.
- `chunk_text` currently uses 500 max tokens with 50-token overlap for library
  and workspace indexing.
- Workspace names must match `^[a-z0-9]+(-[a-z0-9]+)*$`.

## Scenario: Validated workspace path boundary

### 1. Scope / Trigger

This contract applies whenever workspace TOML or workspace RAG sidecar paths
are created, read, deleted, or opened. Raw workspace strings must not reach a
filesystem or SQLite path sink.

### 2. Signatures

- `WorkspaceName::parse(name: &str) -> ZotResult<WorkspaceName>`
- `WorkspaceStore::{create, load, delete, exists}(&WorkspaceName, ...)`
- `WorkspaceStore::save(workspace: &Workspace) -> ZotResult<()>`
- `WorkspaceRagStore::open(store: &WorkspaceStore, name: &WorkspaceName)`

### 3. Contracts

- `WorkspaceName` accepts only `^[a-z0-9]+(-[a-z0-9]+)*$`; path construction
  consumes the validated type rather than `&str`.
- `save()` reparses the public `Workspace.name` field before writing so manual
  struct construction cannot bypass validation.
- TOML and `<name>.idx.sqlite` targets have a canonical parent equal to the
  canonical workspace root. An existing symlink/reparse target resolving outside
  that root fails closed.
- Atomic TOML persistence and the shared `.md_cache.sqlite` layout stay unchanged.

### 4. Validation & Error Matrix

- Empty, traversal, separator, absolute, Windows prefix, uppercase, or malformed
  kebab-case name -> `InvalidInput` code `invalid-workspace-name` before I/O.
- Canonical target outside workspace root -> `InvalidInput` code
  `workspace-path-boundary` before read/open/delete.
- Missing valid file -> normal `Io` error at the requested in-root path.
- Valid name and in-root target -> preserve existing store/RAG behavior.

### 5. Good / Base / Bad Cases

- Good: parse `llm-safety`, then create `llm-safety.toml` and
  `llm-safety.idx.sqlite` under the configured root.
- Base: `list()` ignores malformed legacy filenames and returns valid workspaces
  in deterministic name order.
- Bad: call `root.join(format!("{raw}.toml"))`, or open an existing workspace
  symlink whose target is outside the canonical root.

### 6. Tests Required

- Table-test valid kebab-case and traversal, separators, absolute/drive/UNC,
  uppercase, and malformed dash inputs.
- Prove `save()` rejects a manually constructed invalid `Workspace.name` without
  creating a root-external file.
- On platforms with testable symlinks, prove TOML load and RAG open reject targets
  outside the root; keep valid round-trip and sidecar-layout tests green.

### 7. Wrong vs Correct

```rust
// Wrong: a raw name owns path semantics at every caller.
let path = store.root().join(format!("{name}.idx.sqlite"));

// Correct: parse once and let WorkspaceStore enforce the path boundary.
let name = WorkspaceName::parse(raw)?;
let rag = WorkspaceRagStore::open(&store, &name)?;
```

## Scenario: PDF annotation geometry and text-cache sidecar

### 1. Scope / Trigger

This contract applies when changing area annotation coordinates, `PdfCache`,
cache paths, or PDF text-cache schema/fingerprints. Both the CLI and direct
`PdfBackend` callers must reject invalid geometry, while every cache connection
must tolerate concurrent CLI processes without accepting stale same-metadata
content.

### 2. Signatures

- `validate_area_coordinates(x, y, width, height) -> ZotResult<()>`
- `PdfiumBackend::build_area_position(...) -> ZotResult<PdfAreaPosition>`
- `PdfCache::new(path: Option<PathBuf>) -> ZotResult<PdfCache>`
- SQLite: `cache(cache_key TEXT PRIMARY KEY, content TEXT NOT NULL)`,
  `PRAGMA user_version = 1`.

### 3. Contracts

- Coordinates are finite, `0 <= x,y < 1`, `width,height > 0`, and both rectangle
  endpoints are at most 1. CLI orchestration calls the shared validator before
  local/PDF/remote I/O; Pdfium calls it again before loading a document.
- File-backed PDF caches use WAL, a 5000 ms busy timeout, and explicit
  `user_version` on every open. Version 0 upgrades in place; versions newer than
  the binary fail closed.
- Cache keys are `sha256:<full-content-hex>`, streamed through the existing
  SHA-256 reader. Do not return to path/mtime/length fingerprints.
- Default, library semantic, and workspace shared `.md_cache.sqlite` paths stay
  compatible. Old MD5 rows naturally miss; no destructive migration is needed.

### 4. Validation & Error Matrix

| Condition | Result |
|---|---|
| NaN/Inf, negative origin, non-positive size, or endpoint above 1 | `invalid-annotation-area` before PDF/remote I/O |
| cache open failure | `pdf-cache-open` |
| PRAGMA/table/version migration failure | `pdf-cache-schema` |
| `user_version > 1` | `pdf-cache-schema-version` |
| PDF file read/hash failure | typed `Io` at the PDF path |
| cache lookup/write failure | `pdf-cache-get` / `pdf-cache-put` |

### 5. Good / Base / Bad Cases

- Good: replace a PDF with different bytes while preserving path, length, and
  mtime; the old text is not returned.
- Base: reopen an existing unversioned cache, retain compatible rows, and
  operate in WAL mode with a 5-second busy timeout.
- Bad: validate only in clap/CLI, use MD5 over metadata, silently open a future
  schema, or move the shared cache as part of a behavior fix.

### 6. Tests Required

- Table-test finite/unit-rectangle boundaries, including full-page and
  edge-aligned valid rectangles.
- Reopen a file cache and assert `journal_mode=wal`, `busy_timeout=5000`,
  current `user_version`, and normal put/get.
- Fix mtime and length across a content replacement and assert a miss.
- Create a future-version DB and assert `pdf-cache-schema-version`.
- Run `cargo test -p zot-local`, workspace clippy, and `just ci`.

### 7. Wrong vs Correct

```rust
// Wrong: metadata can be preserved across a byte-for-byte replacement.
let key = md5(format!("{path}:{mtime}:{len}"));

// Correct: content owns cache identity; every backend caller shares validation.
validate_area_coordinates(x, y, width, height)?;
let key = format!("sha256:{}", sha256_reader(File::open(path)?)?.0);
```

## Review Checklist

- Does the change keep Zotero's main database read-only?
- Are user search strings escaped before `LIKE`?
- Are sidecar schema changes migrated and covered by reopen tests?
- Are PDF failures typed as `ZotError::Pdf`?
- Did the change preserve the documented workspace file/index/cache layout?
