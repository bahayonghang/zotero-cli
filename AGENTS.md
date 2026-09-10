# AGENTS.md

## Source of truth
- Prefer `justfile`, root `Cargo.toml`, and `src/zot-cli/src/main.rs` over prose when they differ.
- `skills/zot/SKILL.md` is worth checking for real operator workflows around `doctor`, write actions, and workspaces.

## Workspace map
- This is a Rust workspace (`edition = 2024`, `rust-version = 1.85`) with 5 crates under `src/`:
  - `zot-cli` — binary crate for `zot`; real CLI entrypoint is `src/zot-cli/src/main.rs`
  - `zot-core` — config, models, errors, JSON envelope
  - `zot-local` — local `zotero.sqlite` reads, PDF extraction/cache, workspace/RAG
  - `zot-desktop` — loopback-only Zotero Desktop connector integration
  - `zot-remote` — Zotero Web API writes, Semantic Scholar, embeddings
- Ignore `ref/` when reasoning about the active app: it is a legacy/reference Python implementation, not part of the Rust workspace.

## Dev commands
- Main verification command is `just ci`.
- `just ci` is a pure check alias for `just ci-check`; it runs fmt, locked workspace check,
  clippy, tests, version guards, and canonical-skill mirror checks without rewriting files.
- Use `just version-sync` and `just skills-sync` only when intentionally regenerating versioned
  skill metadata or local mirrors.
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
- Local SQLite (`zot-local`, `zotero.sqlite` plus attachment storage) and Zotero Local HTTP are read-only. Never use either as a write transport.
- The built-in connector (`zot-desktop`) is import-only. The connector imports new BibTeX/RIS into Zotero's currently selected writable target. The connector does not update, merge, tag, note, or mutate collections.
- All other library mutations go through the Zotero Web API via `zot-remote`.
- Never write `zotero.sqlite` directly.
- `zot mcp serve` is scaffolded but currently returns `mcp-not-implemented`; do not build workflows around MCP yet.

## Config and scope quirks
- Config and state paths come from `dirs::config_dir()` (`AppConfig::config_dir` / `config_file` / `state_dir` in `src/zot-core/src/config.rs`). Do not present `~/.config/zot/...` as a universal path.
- Runtime truth is `zot --json doctor` / `zot --json config show` (`doctor.data.config_file`).
- Typical examples (label the platform):
  - Linux/XDG: `~/.config/zot/config.toml`
  - macOS: `~/Library/Application Support/zot/config.toml`
  - Windows: `%AppData%\zot\config.toml` (example: `C:\Users\<user>\AppData\Roaming\zot\config.toml`)
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
- Default workspace root is `AppConfig::state_dir().join("workspaces")` in `src/zot-local/src/workspace.rs`.
- Typical examples (label the platform):
  - Linux/XDG: `~/.config/zot/workspaces`
  - macOS: `~/Library/Application Support/zot/workspaces`
  - Windows: `%AppData%\zot\workspaces`
- Each workspace is stored as `<name>.toml`.
- Index sidecar is `<name>.idx.sqlite`.
- Workspace PDF cache sidecar is `.md_cache.sqlite` in the workspace root.
- Workspace names must be kebab-case (`llm-safety` style).

## Constraints and repo hygiene
- Every workspace member inherits lints that forbid `unsafe`, `dbg!`, `todo!`, and `unwrap()`.
- `.github/workflows/ci.yml` runs the pure gate on Linux, Windows, and macOS, checks Rust 1.85
  MSRV, and includes dependency security and unused-dependency jobs. Local `just ci` remains the
  source of truth for the stable build/test sequence.
- Tests are mostly inline crate tests. `src/zot-cli/tests/` also has integration targets
  (`json_error_contract.rs`, `workspace_version_guard.rs`). `cargo test --workspace` remains
  the expected gate.
- Treat `target/`, `.omx/`, `.claude/`, workspace index files, and PDF cache files as generated state, not source.

## Agent skills

### Agent harnesses

Claude Code, Codex, Grok Build, Kimi Code, and Oh My Pi (OMP) share this file. Claude Code does not auto-read `AGENTS.md`; tracked root `CLAUDE.md` imports this file with `@AGENTS.md`. Five-tool matrix, sources, and evidence levels: `docs/agents/harnesses.md`.

Grok Build and Kimi Code have no Trellis auto platform marker. After `task.py start` and review, implement inline in the main session:
1. Read `.trellis/workflow.md` Phase 2 generic steps.
2. Start the prompt with `Active task: <task path from task.py current>`.
3. Read that task's `prd.md`, `design.md` if present, `implement.md` if present, and relevant specs.
4. Implement inline in the main session. Do not pretend unmatched `[platform]` markers inject context.
5. If session identity is missing, set `TRELLIS_CONTEXT_ID` in the current process only. Do not write the user's global environment.

Planning is not approved implementation. Matrix and sources: `docs/agents/harnesses.md`.

### Issue tracker

Local Trellis `prd.md` / `design.md` / `implement.md` under `.trellis/tasks/` are the implementation-acceptance source for an active task.
GitHub Issues for `bahayonghang/zotero-cli` are authorized collaboration / remote tracking records.
Do not create, comment on, or close GitHub issues without explicit authorization.
See `docs/agents/issue-tracker.md`.

### Triage labels

The repo uses the canonical five-label triage vocabulary. See `docs/agents/triage-labels.md`.

### Domain docs

This repo uses the single-context domain-doc layout. See `docs/agents/domain.md`.

### Performance and behaviour limits

Operational ceilings (semantic search O(N), embedding batch size, LIKE escape
semantics, Scite chunking, PDF outline depth, polite-pool email, envelope
`api_version`). See `docs/agents/limits.md`.
<!-- TRELLIS:START -->
# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:
- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->
