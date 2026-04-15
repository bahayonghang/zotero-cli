# zot (Rust)

Rust workspace for a Zotero CLI that combines:

- local SQLite reads from `zotero.sqlite`
- local PDF and workspace/RAG utilities
- Zotero Web API writes
- Semantic Scholar preprint status checks

## Workspace

All workspace crates live under `src/`:

- `src/zot-core`: shared config, models, errors, JSON envelope
- `src/zot-local`: SQLite reads, PDF, citation export, workspace/RAG
- `src/zot-remote`: Zotero Web API, Semantic Scholar, embedding client
- `src/zot-cli`: `zot` binary and command surface

## Build

```bash
just build
```

Install to Cargo's local bin directory:

```bash
just install
```

## Common Commands

```bash
zot --json doctor
zot --json library search "attention"
zot --json item get ATTN001
zot workspace new my-topic --description "paper set"
zot sync update-status --apply
```

## Config

Config file:

- `~/.config/zot/config.toml`

Environment variables:

- `ZOT_DATA_DIR`
- `ZOT_LIBRARY_ID`
- `ZOT_API_KEY`
- `ZOT_EMBEDDING_URL`
- `ZOT_EMBEDDING_KEY`
- `ZOT_EMBEDDING_MODEL`
- `S2_API_KEY`
- `SEMANTIC_SCHOLAR_API_KEY`

## Status

Implemented:

- command surface for doctor, library, item, collection, workspace, sync
- local search/read/stats/duplicates/related
- local citation export and formatting
- remote item/note/tag/collection writes
- attachment upload flow
- workspace indexing/query with BM25 and optional embedding lookup

Current gap:

- `zot mcp serve` is scaffolded in the command surface but currently returns a structured `unsupported` error until RMCP tool wiring is added.
