# Implementation Plan

1. Extend `zot-core::CliEnvelope::Err` with optional metadata plus constructors for canonical
   `ErrorPayload` values; update byte-exact envelope tests.
2. Add a private `zot-cli::AppError` classifier for embedded `ZotError`, JSON serialization,
   generic runtime, and normalized Clap parse failures.
3. Add global `--verbose`, replace `Cli::parse` with a controlled `try_parse_from` top-level
   flow, preserve native help/version behavior, and emit JSON parse errors with exit status 2.
4. Validate output protocols before `AppContext` construction, rejecting `--json graph serve`
   and `--json completions` without side effects.
5. Centralize error rendering in `format.rs`, including error metadata and opt-in stderr chains;
   keep command handlers and human server output unchanged.
6. Add binary integration goldens covering every top-level group, one-document stdout, parse
   failures, long-running/raw protocol rejection, stable codes, and verbose stderr isolation.
7. Update `zot-cli` and `zot-core` error/logging specs with the executable contract.
8. Run formatting, focused error/envelope tests, all `zot-cli` tests, workspace clippy, then
   `just ci`; inspect stdout, exit statuses, final diff, and later-child exclusions.

## Validation commands

```powershell
cargo test -p zot-core envelope
cargo test -p zot-cli app_error
cargo test -p zot-cli --test json_error_contract
cargo test -p zot-cli
cargo clippy --workspace --all-targets -- -D warnings
just ci
```

## Risk and rollback points

- `src/zot-cli/src/main.rs`: exit status and Clap help/version behavior must remain native.
- `src/zot-cli/src/format.rs`: stdout must contain one complete error document and no diagnostics.
- `src/zot-core/src/envelope.rs`: error metadata is additive; success serialization must remain
  byte-exact.
- Integration tests must not depend on the user's Zotero installation or mutate config/library
  state; all forced failures occur before command handlers.

## Pre-start checks

- PRD convergence pass complete; no unresolved product or scope questions.
- `task.py validate` passes before `task.py start`.
- Only this task directory is staged for the planning commit; later child directories and the
  root audit report remain untouched.
