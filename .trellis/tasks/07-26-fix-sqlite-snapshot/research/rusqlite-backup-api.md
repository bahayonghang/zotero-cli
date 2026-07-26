# rusqlite 0.39 Backup API evidence

## Local source inspection

Inspected the installed crate source at
`~/.cargo/registry/src/.../rusqlite-0.39.0/src/backup.rs` on 2026-07-26.

- The module is gated by rusqlite's `backup` feature.
- `backup::Backup::new(&source, &mut destination)` wraps `sqlite3_backup_init`.
- `Backup::step(pages)` returns `StepResult::{Done, More, Busy, Locked}`; busy and locked are
  explicit successful step results rather than fatal `Err` values.
- `run_to_completion()` retries `More`, `Busy`, and `Locked` without a deadline. The project needs
  a small custom loop so a continuously busy Zotero database fails with a stable error instead of
  hanging indefinitely.
- SQLite forbids use of the destination connection while a backup handle exists; the handle must
  be dropped before destination validation or read-only reopen.

## Project decision

Use the page-step API with a monotonic deadline, not the convenience `Connection::backup()` or
`run_to_completion()`. This preserves WAL-aware SQLite snapshot semantics while enforcing the
QW-05 requirement that lock contention fails explicitly and operationally.

No new crate is required: add `backup` to the existing workspace rusqlite feature list.
