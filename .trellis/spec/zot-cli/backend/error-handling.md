# Error Handling

`zot-cli` uses `anyhow::Result` inside command orchestration and a private
`AppError` at the executable boundary. `AppError` preserves
`zot_core::ZotError` payloads and classifies every other chain before output.

## Scenario: Versioned one-shot error protocol

### 1. Scope / Trigger

- Trigger: adding a command error, changing CLI parsing, emitting JSON, or adding a command that
  owns a raw/long-running stdout protocol.
- Why: agent callers must never receive plain text or multiple documents on a one-shot `--json`
  failure.

### 2. Signatures

```rust
AppError::runtime(error: anyhow::Error) -> AppError
AppError::cli_parse(detail: String) -> AppError
AppError::payload(&self) -> zot_core::ErrorPayload
Cli::validate_output_protocol(&self) -> Result<(), ZotError>
print_error(error, json, verbose, profile) -> anyhow::Result<()>
```

Global flags are `--json`, `--verbose`, `--profile`, and `--library`.

### 3. Contracts

- A one-shot JSON failure writes exactly one document to stdout:

```json
{
  "ok": false,
  "error": { "code": "...", "message": "...", "hint": "..." },
  "meta": { "profile": "work", "api_version": 1 }
}
```

- `profile` and `hint` are optional; `meta.api_version` is always 1.
- Human errors and verbose diagnostics go to stderr. `--verbose` may add source-chain lines to
  stderr but must not alter stdout, the stable code, or the exit status.
- Clap help/version remain documentation output. A real parse failure with explicit `--json`
  returns the envelope with exit status 2.
- `graph serve` is a long-running human protocol and `completions` is a raw script protocol;
  both reject `--json` before context construction or output.

### 4. Validation & Error Matrix

| Condition | Code | Exit | Output |
|---|---|---:|---|
| chain contains `ZotError` | canonical domain code | 1 | canonical payload |
| chain contains raw `serde_json::Error` | `json-serialization` | 1 | top-level message |
| other runtime chain | `runtime-error` | 1 | top-level message |
| Clap parse failure with `--json` | `cli-parse` | 2 | normalized message/hint |
| JSON requested for server/raw protocol | `json-protocol-unsupported` | 1 | reject before I/O |

Search the complete `anyhow` chain before using a generic code. Never stringify a `ZotError`
before classification.

### 5. Good / Base / Bad Cases

- Good: a wrapped `ZotError` retains its domain code and hint; `--verbose` writes only its extra
  causes to stderr.
- Base: a generic runtime error becomes `runtime-error` and still includes API version 1.
- Bad: a handler prints an error and then returns `Err`, `main` branches only on `ZotError`, or a
  server writes lifecycle text before JSON protocol validation.

### 6. Tests Required

- Unit goldens assert domain, generic, serialization, and parse classifications.
- Byte-exact envelope tests assert `meta.api_version == 1` on error.
- Binary integration tests force a failure through every top-level command group and parse all
  stdout with one `serde_json::from_slice` call, which rejects trailing text/documents.
- Protocol tests assert `graph serve` never binds and `completions` never emits script content
  when `--json` is present.
- Verbose tests assert stdout is byte-identical to non-verbose JSON and details appear only on
  stderr.

### 7. Wrong vs Correct

Wrong:

```rust
if let Some(error) = error.downcast_ref::<ZotError>() {
    print_json(error);
} else {
    eprintln!("{error}");
}
```

Correct:

```rust
let error = AppError::runtime(error);
print_error(&error, cli.json, cli.verbose, cli.profile.as_deref())?;
```

## Validation Pattern

- Validate raw CLI strings at the boundary and return `ZotError::InvalidInput`
  with a stable code and hint.
- `util::parse_page_range` returns `page-range` for invalid PDF page spans.
- `util::parse_json_input` accepts either a JSON string or a path, and returns
  `json-input` with a hint.
- `library::create_saved_search` rejects empty condition arrays before remote
  calls with `saved-search-conditions`.

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
- Do not convert `ZotError` into plain strings before it reaches `AppError`.
- Do not print and then return an error for the same failure; let the top-level
  error path print once.
