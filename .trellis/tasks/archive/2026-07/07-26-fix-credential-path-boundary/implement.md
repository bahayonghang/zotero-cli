# Implementation Plan

1. Add `WorkspaceName` parsing and focused valid/invalid table tests in `zot-local`.
2. Convert `WorkspaceStore` path-bearing APIs and `WorkspaceRagStore::open` to typed names;
   centralize canonical root/target containment checks and retain atomic TOML persistence.
3. Migrate CLI workspace commands and local tests to parse/pass `WorkspaceName`; add traversal
   and symlink escape regressions without changing valid on-disk layout.
4. Split authenticated Zotero request construction from external upload request construction;
   enforce HTTPS in production and a loopback-only test exception.
5. Add the dual-server attachment upload regression and scheme rejection tests.
6. Run focused formatting/checks and tests:
   - `cargo fmt --all --check`
   - `cargo test -p zot-remote`
   - `cargo test -p zot-local`
   - `cargo test -p zot-cli workspace`
7. Load `trellis-check`, inspect the full task diff against zot-remote, zot-local, and zot-cli
   Quality Check sections, fix findings, then run `just ci`.
8. Review whether the origin-scoped credential and validated workspace path contracts should be
   captured in `.trellis/spec/` via `trellis-update-spec` before the task commit.

## Risk And Rollback Points

- HTTP fake-server tests must never create a production insecure-upload exception; keep the flag
  behind the existing test-only constructor surface.
- Canonical containment must support a not-yet-created leaf while rejecting an existing symlink
  to root-external data.
- Workspace API signature changes are compile-enforced; `cargo check --workspace` must expose any
  missed call site before commit.
- No task outside `07-26-fix-credential-path-boundary` may be staged with this task's work commit.
