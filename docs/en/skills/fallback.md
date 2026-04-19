# Fallbacks

## Common fallback strategies

### `zot` is not installed

In a development environment, use:

```bash
cargo run -q -p zot-cli -- --json doctor
```

### Write access is missing

Stay in read-only mode and explicitly report missing:

- `ZOT_API_KEY`
- `ZOT_LIBRARY_ID`

If the user explicitly wants the environment fixed, start with:

- `zot config show`
- `zot config init`
- `zot config set`

### Better BibTeX is unavailable

`library citekey` can only fall back to the Extra field. Do not pretend the BBT RPC path succeeded.

### No PDF backend is available

Do not promise PDF text extraction, outlines, or annotation creation.

### Embeddings are unavailable

Continue with:

- `workspace query --mode bm25`
- or `library search`
- or let `hybrid` degrade naturally

### `attach-mode auto` found no PDF

The item may still be created successfully. Report that no open-access PDF was found instead of treating the whole command as failed.

### The user provided a parent item key instead of an attachment key

Say that the attachment key is still missing first.

When needed, inspect:

- `item children`
- then decide whether `item download` is possible

### `zot mcp serve`

It is not usable yet, so do not design workflows around it.
