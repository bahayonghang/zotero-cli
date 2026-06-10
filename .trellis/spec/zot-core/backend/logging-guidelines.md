# Logging Guidelines

This project does not use a logging framework in `zot-core`. The core crate
should remain output-free and return structured errors/data to callers.

## Output Boundary

- `zot-core` should not call `println!`, `eprintln!`, or logging macros.
- Human-readable output belongs in `zot-cli` formatting code.
- JSON output belongs to `CliEnvelope` construction in `zot-cli`, using
  `zot-core` types as data.

## Diagnostics Pattern

Use structured fields instead of logs:

- For recoverable user actions, set `ZotError` `hint`.
- For config display, redact secrets with `redact_secret` in `config.rs`.
- For status output, model the status as a serializable type such as
  `SemanticIndexStatus`, `PdfiumAvailability`, or `LibraryStats`.

## Code Example

`redact_secret` keeps only the last four characters for longer values and uses
`(set)` for very short secrets:

```rust
pub fn redact_secret(value: &str) -> String {
    if value.len() <= 4 {
        return "(set)".to_string();
    }
    format!("***{}", &value[value.len() - 4..])
}
```

## What Not To Expose

- Zotero API keys, embedding keys, Semantic Scholar keys, and configured
  profile secrets.
- Local full paths unless they are necessary for an `Io` error or doctor-style
  diagnostics.
- Raw remote response bodies in core types; remote clients can include body
  snippets in `ZotError::Remote` when needed.
