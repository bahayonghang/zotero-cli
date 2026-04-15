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

Check `doctor` output under `write_credentials`. The usual missing pieces are:

- `ZOT_API_KEY`
- `ZOT_LIBRARY_ID`

Without them, only local read-only analysis is available.

### 2. `library citekey` returns nothing

Check `doctor` for `better_bibtex.available`:

- when available, citekey lookup can use Better BibTeX JSON-RPC
- when unavailable, lookup falls back to citation keys stored in the Extra field

### 3. PDF / outline / annotation actions fail

Check these `doctor` fields:

- `pdf_backend.available`
- `annotation_support.pdf_outline`
- `annotation_support.annotation_creation`

Without a working backend, do not assume PDF text, outlines, or annotation creation are available.

### 4. semantic search is not very semantic

Check:

- `embedding.configured`
- `semantic_index`

Without embeddings, library and workspace retrieval degrade to BM25 or the available mode.

### 5. feeds do not show up

Check `doctor` for `libraries.feeds_available`. Also remember:

- feeds are not targeted through `--library`
- use `library feeds`
- then use `library feed-items <library-id>`

### 6. `attach-mode auto` did not attach a PDF

That is not always an error. `auto` tries the OA cascade in this order:

1. Unpaywall
2. arXiv relation
3. Semantic Scholar
4. PubMed Central

If no open-access PDF exists, the item may still be created successfully.

### 7. How to target a group library

`--library` only supports:

- `user`
- `group:<id>`

### 8. Why MCP is unavailable

Because `zot mcp serve` is still scaffolding only and does not have a usable implementation yet.

## If it still fails

Check, in order:

1. the config file path
2. required environment variables
3. a consistent invocation path
4. the returned `code / message / hint`
