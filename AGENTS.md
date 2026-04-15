# AGENTS.md

## Source of truth
- Prefer `justfile`, root `Cargo.toml`, and `src/zot-cli/src/main.rs` over prose when they differ.
- `skills/zot-skills/SKILL.md` is worth checking for real operator workflows around `doctor`, write actions, and workspaces.

## Workspace map
- This is a Rust workspace (`edition = 2024`, `rust-version = 1.85`) with 4 crates under `src/`:
  - `zot-cli` — binary crate for `zot`; real CLI entrypoint is `src/zot-cli/src/main.rs`
  - `zot-core` — config, models, errors, JSON envelope
  - `zot-local` — local `zotero.sqlite` reads, PDF extraction/cache, workspace/RAG
  - `zot-remote` — Zotero Web API writes, Semantic Scholar, embeddings
- Ignore `ref/` when reasoning about the active app: it is a legacy/reference Python implementation, not part of the Rust workspace.

## Dev commands
- Main verification command is `just ci`.
- `just ci` runs checks in this order: `cargo fmt --all --check` -> `cargo check --workspace` -> `cargo clippy --workspace --all-targets -- -D warnings` -> `cargo test --workspace`.
- Focused commands:
  - `cargo check -p zot-cli`
  - `cargo test -p zot-local`
  - `cargo run -q -p zot-cli -- ...` for one-off CLI runs without installing
- Release/local install:
  - `just build`
  - `just install`

## Runtime habits that matter
- For any new environment, any write action, any PDF extraction issue, any workspace indexing/query issue, or any “why is this broken” report, run `zot --json doctor` first.
- If `zot` is not installed, use `cargo run -q -p zot-cli -- --json doctor`.
- Pick one invocation path for the whole session (`zot ...` if installed, otherwise `cargo run -q -p zot-cli -- ...`) and keep it consistent.
- Prefer `--json` for agent-driven runs. The CLI returns a standard envelope:
  - success: `{"ok": true, "data": ..., "meta": ...}`
  - failure: `{"ok": false, "error": {"code": "...", "message": "...", "hint": "..."}}`

## Read/write boundaries
- Local reads come from Zotero data files via `zot-local` (`zotero.sqlite` + attachment storage).
- Library mutations go through the Zotero Web API via `zot-remote`.
- Never implement writes by touching `zotero.sqlite` directly.
- `zot mcp serve` is scaffolded but currently returns `mcp-not-implemented`; do not build workflows around MCP yet.

## Config and scope quirks
- Config file path is `~/.config/zot/config.toml`.
- Supported env overrides are:
  - `ZOT_DATA_DIR`
  - `ZOT_LIBRARY_ID`
  - `ZOT_API_KEY`
  - `ZOT_EMBEDDING_URL`
  - `ZOT_EMBEDDING_KEY`
  - `ZOT_EMBEDDING_MODEL`
  - `SEMANTIC_SCHOLAR_API_KEY`
  - `S2_API_KEY`
- `--library` only accepts `user` or `group:<id>`.

## Workspace / RAG storage
- Default workspace root is `~/.config/zot/workspaces`.
- Each workspace is stored as `<name>.toml`.
- Index sidecar is `<name>.idx.sqlite`.
- Workspace PDF cache sidecar is `.md_cache.sqlite` in the workspace root.
- Workspace names must be kebab-case (`llm-safety` style).

## Constraints and repo hygiene
- Workspace lints forbid `unsafe`, `dbg!`, `todo!`, and `unwrap()`.
- There is no repo CI workflow checked in under `.github/workflows/`; local `just ci` is the current source of truth.
- Tests are inline crate tests, not a large integration suite; `cargo test --workspace` is still the expected gate.
- Treat `target/`, `.omx/`, `.claude/`, workspace index files, and PDF cache files as generated state, not source.
