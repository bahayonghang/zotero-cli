# Troubleshooting

## One rule first: run doctor

```bash
zot --json doctor
```

If `zot` is not in PATH:

```bash
cargo run -q -p zot-cli -- --json doctor
```

## Common issues

### 1. Cannot write to Zotero

Check the write-credential status from `doctor`. The usual missing pieces are:

- `ZOT_API_KEY`
- `ZOT_LIBRARY_ID`

Without them, only local read-only analysis is available.

### 2. PDF extraction fails

Check the PDF backend status in `doctor`. If no backend is available, do not assume PDF text or annotations can be extracted.

### 3. workspace query is not semantic enough

Check embedding configuration:

- `ZOT_EMBEDDING_URL`
- `ZOT_EMBEDDING_KEY`
- `ZOT_EMBEDDING_MODEL`

Without embeddings, query falls back to BM25 or the available mode.

### 4. How to target a group library

`--library` only supports:

- `user`
- `group:<id>`

### 5. Why MCP is unavailable

Because `zot mcp serve` is still scaffolding only and does not have a usable implementation yet.

## If it still fails

Check, in order:

1. the config file path
2. required environment variables
3. a consistent invocation path
4. the returned `code / message / hint`
