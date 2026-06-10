# Logging Guidelines

This CLI does not use a logging framework. Output is either a structured JSON
envelope for agents or concise human-readable text for terminal users.

## JSON Output

- Use `--json` for agent-driven runs.
- Success output goes through `print_enveloped`.
- Error output goes through `print_error`.
- Do not mix progress text into JSON mode; it breaks the envelope contract.

## Human Output

- Use `format.rs` helpers for repeated display shapes: `print_items`,
  `print_item`, `print_collections`, `print_stats`, `print_workspace`, and
  `print_query_chunks`.
- Command-specific one-line confirmations are acceptable only in non-JSON
  mode, as seen in workspace and library indexing commands.
- Generic errors should be printed once by `main.rs`, not by every command.

## Doctor Output

Doctor is the main diagnostic surface. In JSON mode it returns a payload with
config, database, write credentials, PDF backend, Better BibTeX, semantic
index, and annotation support status. In human mode it may print a banner and
short status text.

## Code Example

Branch output on `ctx.json`:

```rust
if ctx.json {
    print_enveloped(ctx, &items, None)?;
} else {
    print_items(&items);
}
```

## What Not To Print

- API keys, embedding keys, Semantic Scholar keys, or unredacted config.
- Full PDF text, notes, annotations, or Zotero metadata unless the command
  explicitly returns that content.
- Network progress from `zot-remote` or SQL progress from `zot-local`.
