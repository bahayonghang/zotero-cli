# Implementation Plan

1. Add pinned `sha2` and `fs4` workspace dependencies and extend all seven
   `PdfiumDownloadTarget` entries with archive/library SHA-256.
2. Split managed target-path construction from verified candidate discovery; hash managed
   libraries before they enter `candidate_library_paths`.
3. Replace `download_archive_bytes` with bounded streaming into a same-directory temp file,
   checking content length, actual bytes, sync, and archive digest.
4. Replace direct tar `unpack` with expected-entry-only bounded copy into a second temp file;
   require a regular file, sync it, verify library digest, then atomically persist.
5. Wrap installation in an `fs4` lock and recheck final state after acquiring it; expose a narrow
   download closure seam for deterministic concurrent tests without live network access.
6. Add focused regressions for manifest exactness, old/invalid cache rejection, tampered,
   truncated, wrong-platform, oversize, missing/non-file entry, library mismatch, preservation of
   an existing valid artifact, and concurrent single-download behavior.
7. Run `cargo fmt --all --check`, `cargo test -p zot-local`, relevant `zot-cli` doctor tests, and
   `cargo check --workspace`.
8. Load `trellis-check`, inspect the full diff against zot-local and zot-cli Quality Check
   sections, fix findings, update the Pdfium code-spec via `trellis-update-spec`, then run
   `just ci`.

## Risk And Rollback Points

- Official release hashes must match the persisted research table byte-for-byte; a wrong digest
  disables auto-download rather than weakening validation.
- Windows cannot atomically replace an existing file with `NamedTempFile::persist`; the installer
  uses a content-addressed final name and removes only an invalid same-name file after both new
  hashes pass.
- `fs4` is advisory; every zot installer path must take the same lock, while discovery still
  verifies content independently.
- No other audit-remediation child or the root audit report may be staged with this task.
