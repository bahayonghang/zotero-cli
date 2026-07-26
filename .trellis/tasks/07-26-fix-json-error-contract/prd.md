# P1: JSON error contract unification

## Goal

Make the CLI's one-shot `--json` interface a versioned, machine-safe protocol: every runtime
failure that reaches the top-level boundary must produce exactly one JSON error envelope on
stdout with a stable code and `meta.api_version == 1`, while human output remains concise and
full error chains are exposed only when the caller explicitly passes `--verbose`.

## Background

- The audit report QW-04 (`zotero-cli-code-audit-2026-07-25.md:69-70,122,538-555,720-726`)
  confirms that `src/zot-cli/src/main.rs:17-25` envelopes only errors downcastable to
  `ZotError`; generic `anyhow::Error` values fall back to plain stderr even under `--json`.
- The same report identifies direct output from
  `src/zot-cli/src/commands/graph/server.rs:38-56` as a protocol bypass and requires forced-error
  coverage for every top-level command group plus single-document stdout parsing.
- `src/zot-core/src/envelope.rs` currently carries optional metadata only on success, although
  success output already sets `meta.api_version` through `CommandOutput`.
- The parent task maps all QW-04 work to this child and requires it to finish before
  `07-26-fix-batch-write-gate`, whose partial-failure codes depend on this stable boundary.

## Requirements

### R1 Unified runtime error classification

- Introduce a CLI-owned application error boundary that accepts every `anyhow::Error` chain.
- Preserve the canonical `ZotError::payload()` code, message, and hint whenever a `ZotError`
  occurs anywhere in the chain.
- Classify non-domain JSON serialization failures as `json-serialization`; classify all other
  generic runtime failures as `runtime-error`. Codes are stable and kebab-case.
- Normal human and JSON output expose only the top-level message and actionable hint. The source
  chain is written to stderr only when global `--verbose` is present; it must never be inserted
  into the JSON document or leak secrets by default.

### R2 Versioned error envelope

- Extend `CliEnvelope::Err` with optional `meta` and provide an error-payload constructor so the
  CLI can envelope domain and generic errors through the same path.
- Every executed one-shot command failure under `--json` writes exactly one JSON document to
  stdout with `ok: false`, canonical `error`, and `meta.api_version == 1`.
- Error serialization and printing happen once at the top-level boundary. Command handlers must
  not print an error and then return it.
- Parse failures with an explicit `--json` flag use code `cli-parse`, the same envelope/meta
  contract, and exit status 2. Clap help/version remain Clap-owned documentation output rather
  than command execution results.

### R3 Explicit non-envelope protocols

- `graph serve` remains a human long-running server protocol. `--json graph serve` must fail
  before opening the database or binding a listener with stable code
  `json-protocol-unsupported` and a hint to omit `--json`.
- `completions` remains a raw completion-script protocol. `--json completions ...` must fail
  before writing script bytes with the same stable code and a command-specific message.
- Existing human `graph serve` and `completions` behavior remains unchanged. No JSONL server
  protocol is introduced in this task.

### R4 Compatibility and scope

- Keep the public success envelope shape and API version at 1; adding error metadata is backward
  compatible and does not rename existing `error` fields or domain codes.
- Do not replace `anyhow` throughout command handlers, add an application/use-case layer, build
  an MCP protocol, or introduce a logging dependency.
- Add no production dependency; tests use `std::process::Command` and the Cargo-provided binary.

## Acceptance Criteria

- [ ] Byte-exact golden tests cover domain, generic runtime, JSON serialization, and CLI parse
      errors; every error envelope contains `meta.api_version: 1`.
- [ ] Integration tests force a failure through each top-level command group (`doctor`, `config`,
      `library`, `item`, `collection`, `graph`, `workspace`, `sync`, `mcp`, `completions`) and
      prove stdout parses once as one JSON value, contains no trailing document/text, and has a
      stable code.
- [ ] `--json graph serve` and `--json completions` fail before long-running/raw output begins;
      human invocations retain their declared protocols.
- [ ] Default error output omits source-chain diagnostics; `--verbose` emits chain context only
      on stderr while stdout remains the same single JSON document.
- [ ] Focused `zot-cli`/`zot-core` tests and final `just ci` pass; final diff contains no changes
      from later audit-remediation children.

## Out Of Scope

- A JSONL protocol for long-running commands, MCP implementation, streaming progress events, or
  a general logging/telemetry framework.
- Replacing all internal `anyhow::Result` signatures or creating the report's long-term
  application/use-case architecture.
- Batch mutation result semantics; those belong to the immediately following
  `07-26-fix-batch-write-gate` task.
