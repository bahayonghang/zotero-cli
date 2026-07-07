# Logging Guidelines

This CLI does not use a logging framework. Output is either a structured JSON
envelope for agents or concise human-readable text for terminal users.

## JSON Output

- Use `--json` for agent-driven runs.
- Handlers return a `CommandOutput` (see `output.rs`); they do not print success
  output themselves. The dispatch layer (`commands/mod.rs`) calls `emit()` once.
- `CommandOutput::new` is the single json-vs-human decision point and the single
  place the success envelope + meta are assembled. Do not reintroduce
  `if ctx.json` branches in command modules.
- Error output goes through `print_error`, driven from `main.rs`.
- Do not mix progress text into JSON mode; it breaks the envelope contract.

## Text Export Formats

Commands that export text (`workspace export`, `item export`) follow a dual
contract:

- Default (no `--json`) mode emits the bare export text (bibtex/markdown) or a
  bare pretty-printed JSON array for `-f json`, suitable for shell pipelines.
- `--json` mode wraps textual exports in the standard envelope with a
  `{ "format": ..., "content": ... }` payload, so agents get a uniform,
  parseable shape regardless of export format.

## Human Output

- Use `format.rs` helpers for repeated display shapes: `print_items`,
  `print_item`, `print_collections`, `print_stats`, `print_workspace`, and
  `print_query_chunks`. Pass them as the `CommandOutput::new` human closure.
- Command-specific one-line confirmations are acceptable only in non-JSON
  mode, as seen in workspace and library indexing commands.
- Generic errors should be printed once by `main.rs`, not by every command.

## Doctor Output

Doctor is the main diagnostic surface. In JSON mode it returns a payload with
config, database, write credentials, PDF backend, Better BibTeX, semantic
index, and annotation support status. In human mode it may print a banner and
short status text.

## Code Example

Return a `CommandOutput`; the dispatch layer prints it:

```rust
let items = library.search(...)?.items;
CommandOutput::new(ctx, items, seed, |items| print_items(items))
```

The json branch serializes `items` into the envelope; the human branch runs the
closure. Neither the handler nor the closure touches `ctx.json`.

## What Not To Print

- API keys, embedding keys, Semantic Scholar keys, or unredacted config.
- Full PDF text, notes, annotations, or Zotero metadata unless the command
  explicitly returns that content.
- Network progress from `zot-remote` or SQL progress from `zot-local`.
