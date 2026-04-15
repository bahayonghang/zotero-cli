# Fallbacks

## Common fallback strategies

### `zot` is not installed

Use:

```bash
cargo run -q -p zot-cli -- --json doctor
```

### Write access is missing

Stay in read-only mode and explicitly report missing:

- `ZOT_API_KEY`
- `ZOT_LIBRARY_ID`

### No PDF backend is available

Do not promise PDF text or annotation extraction.

### Embeddings are unavailable

Continue with:

- `workspace query --mode bm25`
- or let `hybrid` degrade naturally

### `zot mcp serve`

It is not usable yet, so do not design workflows around it.

### `item create --pdf` cannot extract a DOI

Ask for an explicit `--doi` instead of guessing metadata.
