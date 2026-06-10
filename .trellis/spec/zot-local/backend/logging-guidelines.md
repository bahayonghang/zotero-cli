# Logging Guidelines

`zot-local` should not print or log during normal library operations. It is a
library crate used by `zot-cli`; return structured data and `ZotError` values
instead.

## Output Boundary

- Human output belongs in `zot-cli/src/format.rs` and command handlers.
- JSON output belongs in `zot-cli` via `print_enveloped`.
- Local status should be represented as serializable data, for example
  `SemanticIndexStatus` and `PdfiumAvailability`.

## Diagnostics Pattern

- Use precise error codes and hints instead of logs.
- For doctor-style checks, expose status functions. `PdfiumBackend::status`
  reports availability, cached state, auto-download support, and a note.
- For semantic status, use `SemanticStore::status_at` when the index might not
  exist and should not be created as a side effect.

## What Not To Expose

- Full extracted PDF text in diagnostics unless the user explicitly asked for
  text extraction.
- API keys or config secrets. `zot-local` should not handle those directly.
- Raw Zotero database internals in user-facing messages unless needed to debug
  schema compatibility.

## Tests

Some tests use `eprintln!` for diagnostics in other crates, but `zot-local`
runtime code should stay output-free. If a new local function seems to need a
log line, return a status payload or a typed error instead.
