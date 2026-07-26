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

## Review Checklist

- Does the change keep Zotero's main database read-only?
- Are user search strings escaped before `LIKE`?
- Are sidecar schema changes migrated and covered by reopen tests?
- Are PDF failures typed as `ZotError::Pdf`?
- Did the change preserve the documented workspace file/index/cache layout?
