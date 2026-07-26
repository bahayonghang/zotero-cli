# Implementation Plan

1. Enable the existing rusqlite `backup` feature at the workspace dependency.
2. Add `LibrarySnapshotMeta`, private snapshot policy, busy/error mapping, source read-only open,
   paged Backup API copy, read-only reopen, and `quick_check` validation in `db.rs`.
3. Store snapshot metadata and `TempDir` on `LocalLibrary`; export the metadata type from
   `zot-local` without changing existing method signatures.
4. Add focused `db.rs` tests for WAL visibility, bounded lock failure, metadata, and concurrent
   cross-table invariants; keep fixture-backed search tests green.
5. Add doctor payload/human output plus focused JSON shape tests.
6. Update `.trellis/spec/zot-local/backend/database-guidelines.md` with the complete executable
   snapshot contract and update CLI logging/error specs only if their contract changes.
7. Run `cargo fmt --all`, focused zot-local snapshot tests, `cargo test -p zot-local`, focused
   doctor tests, workspace clippy, then `just ci`.
8. Inspect the final diff for source writes, URI/immutable remnants, manual WAL/SHM copying,
   unbounded retry, temporary-path exposure, and unrelated task files.

## Risk and rollback points

- `src/zot-local/src/db.rs`: backup lifecycle and lock timing; revert with the dependency feature.
- `src/zot-cli/src/commands/doctor.rs`: additive output contract; keep existing fields stable.
- Windows: destination must be closed before read-only reopen and TempDir cleanup must occur only
  after `LocalLibrary` drops.

## Validation commands

```powershell
cargo test -p zot-local snapshot
cargo test -p zot-local
cargo test -p zot-cli doctor
cargo clippy --workspace --all-targets -- -D warnings
just ci
```

## Pre-start checks

- PRD convergence pass complete; no open product questions.
- `task.py validate` passes before `task.py start`.
- Only this task directory may be added during planning; other child tasks and the root audit
  report remain untouched.
