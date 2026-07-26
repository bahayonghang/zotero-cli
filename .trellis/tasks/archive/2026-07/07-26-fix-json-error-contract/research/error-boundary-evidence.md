# Error boundary evidence

## Confirmed live paths

- `src/zot-cli/src/main.rs:17-25`: `Cli::parse()` owns parse exits; runtime failures are JSON
  only when `anyhow::Error::downcast_ref::<ZotError>()` succeeds, otherwise they are plain stderr.
- `src/zot-cli/src/format.rs`: `print_error` accepts only `ZotError`; success API version 1 is
  already centralized through `CommandOutput` and `EnvelopeMeta`.
- `src/zot-core/src/envelope.rs`: `CliEnvelope::Ok` has optional metadata; `Err` does not.
- `src/zot-cli/src/commands/graph/server.rs:38-56`: the long-running server intentionally writes
  lifecycle text directly. It cannot share the one-shot envelope without a separate protocol.
- `src/zot-cli/src/commands/mod.rs:29-33`: completions intentionally write a raw shell script to
  stdout before returning a silent output.

## Existing primitives

- `anyhow::Error::downcast_ref` searches its chain, so wrapped `ZotError` and
  `serde_json::Error` can be classified without replacing handler signatures.
- Clap's `try_parse_from` exposes `ErrorKind::DisplayHelp` / `DisplayVersion`, error exit codes,
  and controlled printing, allowing JSON parse failures without changing native documentation
  actions.
- Cargo sets `CARGO_BIN_EXE_zot` for integration tests; the standard library process API is
  sufficient and avoids a new test dependency.

## Parent and report mapping

- Parent `prd.md:27,40,45-50` assigns QW-04 wholly to this child and orders it before batch write
  gating.
- Audit report `:720-726` requires all top-level errors to use envelopes, stable generic codes,
  and one-document stdout; `:959-963` adds command-group failure and API-version coverage.
- Application/use-case architecture, mutation ledgers, MCP implementation, and JSONL streaming
  remain outside this child and the parent's current scope.
