# workspace command

`workspace` manages topic-based paper sets and supports local search plus RAG-style query.

## Storage conventions

- Default root: `~/.config/zot/workspaces`
- Workspace file: `<name>.toml`
- Index sidecar: `<name>.idx.sqlite`
- PDF cache sidecar: `.md_cache.sqlite`
- Name format: kebab-case, for example `llm-safety`

## Subcommands

```bash
zot --json workspace new llm-safety --description "LLM safety papers"
zot --json workspace list
zot --json workspace show llm-safety
zot --json workspace add llm-safety ATTN001 ATTN002
zot --json workspace remove llm-safety ATTN001
zot --json workspace import llm-safety --collection COLL001
zot --json workspace import llm-safety --tag safety
zot --json workspace import llm-safety --search "reward hacking"
zot --json workspace search llm-safety "alignment"
zot workspace export llm-safety --format markdown
zot workspace export llm-safety --format json
zot workspace export llm-safety --format bibtex
zot --json workspace index llm-safety
zot --json workspace query llm-safety "What are the main causes of reward hacking?" --mode hybrid --limit 5
```

## query modes

- `bm25`
- `semantic`
- `hybrid`

Notes:

- do not assume `semantic` or `hybrid` is meaningful when embeddings are unavailable
- workspace query and library semantic search reuse the same local index implementation

## Recommended workflow

1. `workspace new`
2. `workspace import` or `workspace add`
3. `workspace index`
4. `workspace query`

This is the right flow for a long-lived topic set that you want to keep querying over time.
