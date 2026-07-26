# Design: unified CLI error boundary

## Architecture and flow

The executable owns protocol selection and error classification:

```text
argv
  -> detect explicit --json / --verbose for parse-failure handling
  -> Cli::try_parse_from
       -> help/version: native Clap output
       -> parse error + --json: AppError(cli-parse) -> one envelope -> exit 2
       -> parse error human: native Clap output -> exit 2
  -> validate one-shot JSON protocol
       -> reject graph serve / completions under --json
  -> AppContext + dispatch
       -> any anyhow chain -> AppError classification
  -> render once (stdout JSON or stderr human; optional verbose chain on stderr)
```

`AppError` is private to `zot-cli`. It retains an `anyhow::Error` for runtime failures and can
also represent a normalized CLI parse failure. Its `payload()` method is the only classifier:

1. embedded `ZotError` -> canonical `ZotError::payload()`;
2. embedded `serde_json::Error` -> `json-serialization`;
3. all remaining `anyhow` chains -> `runtime-error`;
4. normalized Clap failure -> `cli-parse`.

## Envelope contract

`zot-core` remains the schema owner. `CliEnvelope::Err` gains the same optional `meta` field as
the success variant and accepts an `ErrorPayload` through `err_payload_with_meta`. The existing
`err(&ZotError)` constructor remains available for source compatibility, but the CLI top-level
path always supplies:

```json
{
  "ok": false,
  "error": { "code": "...", "message": "...", "hint": "..." },
  "meta": { "profile": "...", "api_version": 1 }
}
```

`profile` is the parsed requested profile when available and otherwise omitted. The API version
constant stays owned by `zot-cli`; this task does not bump it because existing fields retain
their meaning and error metadata is additive.

## Output protocols

| Surface | Human mode | `--json` mode |
|---|---|---|
| one-shot commands | existing human renderer/errors | one success or error envelope |
| `graph serve` | long-running status on stdout, diagnostics on stderr | rejected before I/O |
| `completions` | raw shell script on stdout | rejected before script generation |
| help/version | native Clap documentation output | native Clap documentation output |

Help/version are parser documentation actions, not executed command results. No listener,
database, script, or partial success output may occur before protocol validation.

## Diagnostics and secrecy

The regular payload contains only the existing domain message/hint or the generic top-level
message. `--verbose` prints the remaining error chain to stderr after the primary output. In
JSON mode this preserves stdout byte-for-byte and keeps parsers isolated from diagnostics.
Verbose is opt-in and must not change codes, exit status, or envelope data.

## Testing strategy

- Unit goldens exercise classification and byte-exact envelope serialization without capturing
  process-global stdout.
- Binary integration tests use `env!("CARGO_BIN_EXE_zot")`. A syntactically valid invocation
  for every top-level group uses invalid `--library` to force the same typed pre-dispatch error;
  protocol-owning commands assert `json-protocol-unsupported` instead.
- Separate process tests cover Clap parse failure and JSON plus verbose separation. Each test
  parses all stdout using `serde_json::from_slice`, which rejects trailing non-whitespace bytes.

## Compatibility and rollback

Human command behavior and all existing domain error codes remain stable. Scripts consuming
plain stderr from generic `--json` failures must migrate to the documented envelope, which is
the intended repair. Revert the CLI, envelope, tests, and spec changes as one unit; do not leave
the error metadata schema without the matching top-level renderer.
