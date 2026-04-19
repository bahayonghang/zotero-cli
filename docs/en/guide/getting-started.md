# Getting Started

## Keep this framing in mind

The primary interface in this repository is `zot-skills`, not the subcommand list.

- `zot-skills` decides what kind of Zotero content the user is asking for
- Rust `zot` performs the actual reads, retrieval, indexing, and writes
- The CLI pages remain reference material for debugging, scripts, and direct invocation

If the agent is supposed to find papers, read PDFs, extract annotations, build a topic workspace, or update Zotero items, start from the skill-first path.

If you are migrating from `ref/zotero-cli`, read [Migrating from ref\\zotero-cli](/en/guide/migrating-from-ref-zotero-cli) first.
If you are migrating from `ref/zotagent`, also read [Migrating from ref\\zotagent](/en/guide/migrating-from-ref-zotagent).

## What the skill can surface from Zotero

- Item metadata: title, creators, year, item type, citation, child items
- Evidence: PDF full text, outline, annotations, notes
- Organization: tags, collections, libraries, feeds
- Working sets: workspace creation, semantic indexing, semantic query/search
- Controlled writes: notes, tags, collections, imports, duplicate merge, publication-status sync

## Recommended startup sequence

### 1. Install the skill

```bash
npx skills add https://github.com/bahayonghang/zotero-cli --skill zot-skills
```

### 2. Provide the runtime

```bash
cargo install --git https://github.com/bahayonghang/zotero-cli.git zot-cli --locked
```

### 3. Run one doctor check

```bash
zot --json doctor
```

If you are developing inside this repository and `zot` is not installed on `PATH`:

```bash
cargo run -q -p zot-cli -- --json doctor
```

Pick one invocation path and keep it consistent for the session.

### 4. Configure writes and saved-search support when needed

If you plan to:

- write notes, tags, or collection membership
- create or delete saved searches
- run publication-status sync

initialize config first:

```bash
zot config init --library-id <your library id> --api-key <your api key>
```

If you want a separate named profile:

```bash
zot config init --target-profile work --library-id <your library id> --api-key <your api key> --make-default
```

## What users can ask directly

- “Find papers in my library about reward hacking.”
- “Pull the PDF annotations and notes for this paper.”
- “Create an `llm-safety` workspace and import the relevant papers.”
- “Check whether this preprint has an official publication record now.”
- “Add a note and a `priority` tag to this item.”

These are all first-class `zot-skills` requests.

For fuller phrasing patterns, read [Agent Usage](/en/skills/agent-usage).

## When to drop to direct commands

Direct runtime calls are more appropriate when:

- you are debugging the environment
- you want to verify a specific subcommand response
- you are writing scripts or regression tests
- you need to inspect `doctor`, indexing, or write prerequisites explicitly

Common starting points:

```bash
zot --json doctor
zot --json library search "reward hacking" --limit 10
zot --json item get ATTN001
zot --json item annotation list --item-key ATTN001
zot --json workspace query llm-safety "What are the main failure modes?" --mode hybrid --limit 5
```

## When to run doctor first

Run `doctor` first when:

- you are in a new environment
- you are about to mutate the library
- the task depends on PDF / outline / annotation support
- you want library semantic indexing or search
- workspace indexing or query is failing
- you are doing Better BibTeX citekey lookup
- the user says “why is this broken”

Pay special attention to:

- `db_exists`
- `write_credentials.configured`
- `pdf_backend.available`
- `better_bibtex.available`
- `libraries.feeds_available`
- `semantic_index`
- `annotation_support`
- `embedding.configured`

## Repository commands

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

- [Agent Usage](/en/skills/agent-usage)
- [Skills Overview](/en/skills/overview)
- [Workflows](/en/skills/workflows)
- [Routing](/en/skills/routing)
- [Migrating from ref\zotero-cli](/en/guide/migrating-from-ref-zotero-cli)
- [Migrating from ref\zotagent](/en/guide/migrating-from-ref-zotagent)
- [CLI Overview](/en/cli/overview)
