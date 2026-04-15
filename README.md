# zot

[English](./README.md) | [简体中文](./README.zh-CN.md) | [Docs (EN)](./docs/en/index.md) | [文档（中文）](./docs/index.md)

`zot` is a Rust-first Zotero CLI for local library reads, PDF extraction, workspace retrieval, and authenticated Zotero Web API writes. It is designed as a CLI-first substitute for the older `ref/zotero-mcp` capability surface, while keeping the workflow centered on `library`, `item`, `collection`, `workspace`, and `sync`.

## Highlights

- Read local Zotero data from `zotero.sqlite` and attachment storage
- Search by query, exact tag, creator, year, type, collection, or citation key
- Enumerate tags, libraries, feeds, and feed items
- Extract PDF text, annotations, and outlines; inspect item children in one call
- Build library-level semantic indexes and run semantic or hybrid search
- Create items from DOI, URL, or file with `attach_mode`-controlled OA PDF attachment
- Manage notes, tags, collections, duplicate merge flows, and Scite checks
- Maintain local reading workspaces with BM25, semantic, or hybrid retrieval

## Workspace Layout

The Rust workspace lives under `src/`:

- `src/zot-core`: shared config, models, errors, JSON envelope
- `src/zot-local`: SQLite reads, PDF helpers, workspace and local index logic
- `src/zot-remote`: Zotero Web API, Better BibTeX, OA PDF resolution, Scite, embeddings
- `src/zot-cli`: the `zot` binary and command surface

## Quick Start

Build and install:

```bash
just build
just install
```

Run a first environment check:

```bash
zot --json doctor
```

If `zot` is not installed yet:

```bash
cargo run -q -p zot-cli -- --json doctor
```

## Common Commands

```bash
zot --json doctor
zot --json library search "attention" --tag transformer --creator Vaswani --year 2017
zot --json library citekey Smith2024
zot --json library semantic-status
zot --json library semantic-index --fulltext
zot --json library semantic-search "mechanistic interpretability" --mode hybrid --limit 5
zot --json item get ATTN001
zot --json item children ATTN001
zot --json item outline ATTN001
zot --json item add-doi 10.1038/nature12373 --tag reading --attach-mode auto
zot --json item annotation list --item-key ATTN001
zot --json item scite report --item-key ATTN001
zot --json collection search Transform
zot --json workspace query llm-safety "What are the main failure modes?" --mode hybrid --limit 5
```

## Configuration

Config file:

- `~/.config/zot/config.toml`

Common environment variables:

- `ZOT_DATA_DIR`
- `ZOT_LIBRARY_ID`
- `ZOT_API_KEY`
- `ZOT_EMBEDDING_URL`
- `ZOT_EMBEDDING_KEY`
- `ZOT_EMBEDDING_MODEL`
- `SEMANTIC_SCHOLAR_API_KEY`
- `S2_API_KEY`

Optional integration and test overrides:

- `ZOT_BBT_PORT`
- `ZOT_BBT_URL`
- `ZOT_SCITE_API_BASE`
- `ZOT_CROSSREF_API_BASE`
- `ZOT_UNPAYWALL_API_BASE`
- `ZOT_PMC_API_BASE`
- `ZOT_SEMANTIC_SCHOLAR_GRAPH_BASE`

## Docs

- Getting started: [docs/en/guide/getting-started.md](./docs/en/guide/getting-started.md)
- CLI overview: [docs/en/cli/overview.md](./docs/en/cli/overview.md)
- Skill routing and safety: [docs/en/skills/overview.md](./docs/en/skills/overview.md)
- Chinese docs home: [docs/index.md](./docs/index.md)

## Current Boundary

- `zot mcp serve` is still scaffold-only and returns an unsupported result
- The old MCP connector-style `search` / `fetch` tools are intentionally mapped onto CLI workflows instead of separate commands
- Annotation creation is PDF-first and depends on local PDF availability plus write credentials

## Verification

Repository validation runs through:

```bash
just ci
```
