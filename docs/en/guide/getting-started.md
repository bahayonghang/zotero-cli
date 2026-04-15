# Getting Started

## What this project is

This repository is a Rust workspace for a CLI-first Zotero toolchain:

- read local `zotero.sqlite` data and attachment storage
- extract PDF text, outlines, and annotations
- perform authenticated writes through the Zotero Web API
- run library-level semantic index/search and workspace retrieval
- support Better BibTeX citation-key lookup, Scite checks, and preprint status sync

The command surface is defined by `src/zot-cli/src/main.rs`.

## Two invocation paths

Pick one invocation path and keep it consistent for a session:

```bash
zot --json doctor
```

If `zot` is not installed yet:

```bash
cargo run -q -p zot-cli -- --json doctor
```

## When to run doctor first

Run `doctor` first when:

- you are in a new environment
- you are about to mutate the library
- the task depends on PDF / outline / annotation support
- you want library semantic indexing or search
- workspace indexing or query is failing
- you are doing citation-key lookup through Better BibTeX
- the user says “why is this broken”

Recommended command:

```bash
zot --json doctor
```

Pay special attention to:

- `write_credentials.configured`
- `pdf_backend.available`
- `better_bibtex.available`
- `libraries.feeds_available`
- `semantic_index`
- `annotation_support`
- `embedding.configured`

## Build and install

Common repository commands from the root:

```bash
just build
just install
just ci
```

`just ci` runs, in order:

1. `cargo fmt --all --check`
2. `cargo check --workspace`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace`

## Configuration

- Config file: `~/.config/zot/config.toml`
- Workspace root: `~/.config/zot/workspaces`

Common environment variables:

- `ZOT_DATA_DIR`
- `ZOT_LIBRARY_ID`
- `ZOT_API_KEY`
- `ZOT_EMBEDDING_URL`
- `ZOT_EMBEDDING_KEY`
- `ZOT_EMBEDDING_MODEL`
- `SEMANTIC_SCHOLAR_API_KEY`
- `S2_API_KEY`

Optional integration overrides:

- `ZOT_BBT_PORT`
- `ZOT_BBT_URL`
- `ZOT_SCITE_API_BASE`
- `ZOT_CROSSREF_API_BASE`
- `ZOT_UNPAYWALL_API_BASE`
- `ZOT_PMC_API_BASE`
- `ZOT_SEMANTIC_SCHOLAR_GRAPH_BASE`

## Preview the docs site locally

The docs site lives in `docs/` and uses VitePress:

```bash
cd docs
npm install
npm run dev
```

Production build:

```bash
cd docs
npm run build
```

## Read next

- [CLI Overview](/en/cli/overview)
- [library command](/en/cli/library)
- [item command](/en/cli/item)
- [Skills Overview](/en/skills/overview)
