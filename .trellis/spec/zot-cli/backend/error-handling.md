# Error Handling

`zot-cli` uses `anyhow::Result` at command boundaries while preserving
`zot_core::ZotError` for structured user-facing failures.

## Top-Level Boundary

`main.rs` parses the CLI, runs the command, and handles errors:

- If the error chain contains `ZotError`, call `print_error(zot_error, json)`
  and exit with status 1.
- Otherwise print the generic error to stderr and exit with status 1.

This keeps command handlers ergonomic while preserving the JSON envelope for
known application failures.

## Validation Pattern

- Validate raw CLI strings at the boundary and return `ZotError::InvalidInput`
  with a stable code and hint.
- `util::parse_page_range` returns `page-range` for invalid PDF page spans.
- `util::parse_json_input` accepts either a JSON string or a path, and returns
  `json-input` with a hint.
- `library::create_saved_search` rejects empty condition arrays before remote
  calls with `saved-search-conditions`.

## JSON Error Contract

When `--json` is set, errors print as:

```json
{
  "ok": false,
  "error": {
    "code": "...",
    "message": "...",
    "hint": "..."
  }
}
```

The success side always goes through `print_enveloped`, which includes
`meta.api_version == 1`.

## Code Example

Command-local validation should preserve the `ZotError` type:

```rust
serde_json::from_str(&raw).map_err(|err| ZotError::InvalidInput {
    code: "json-input".to_string(),
    message: format!("Invalid JSON for {label}: {err}"),
    hint: Some("Pass a JSON string or a path to a JSON file".to_string()),
})
```

## Avoid

- Do not use `panic!` or `unwrap()` in runtime command code.
- Do not convert `ZotError` into plain strings before it reaches `main.rs`.
- Do not print and then return an error for the same failure; let the top-level
  error path print once.
