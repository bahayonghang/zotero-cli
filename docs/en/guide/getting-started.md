# Getting Started

## What this project contains

This repository is a Rust workspace that provides a Zotero-focused CLI for:

- reading local `zotero.sqlite`
- reading PDFs and workspace content
- performing writes through the Zotero Web API
- indexing/querying workspaces and syncing preprint status

The real command entrypoint is `src/zot-cli/src/main.rs`, and this documentation follows that source.

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
- you are about to perform writes
- PDF extraction is failing
- workspace indexing or query is failing
- the user reports “why is this broken”

Recommended command:

```bash
zot --json doctor
```

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

## Configuration locations

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

## Run the docs site locally

The docs site itself lives in `docs/` and uses VitePress:

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
- [Skills Overview](/en/skills/overview)
- [Troubleshooting](/en/cli/troubleshooting)
