# zot-core Backend Guidelines

`zot-core` owns the shared contracts used by every other crate: configuration
loading, library scope parsing, domain models, typed errors, and the JSON
envelope. It should stay dependency-light and must not grow CLI command
handling, SQLite access, HTTP clients, PDF extraction, or user-facing output.

## Pre-Development Checklist

- Read [Directory Structure](./directory-structure.md) before adding or moving
  shared types.
- Read [Error Handling](./error-handling.md) before adding a new error code,
  envelope field, or validation helper.
- Read [Quality Guidelines](./quality-guidelines.md) before changing public
  model serialization or workspace dependencies.
- Read [Database Guidelines](./database-guidelines.md) when a change is tempted
  to add persistence to `zot-core`; the answer is normally to keep persistence
  in `zot-local`, `zot-remote`, or `zot-cli`.
- Read [Logging Guidelines](./logging-guidelines.md) before adding diagnostics
  or exposing secrets.

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | Shared module ownership and public re-exports | Complete |
| [Database Guidelines](./database-guidelines.md) | Persistence boundaries for the core crate | Complete |
| [Error Handling](./error-handling.md) | `ZotError`, `ZotResult`, and envelope payload rules | Complete |
| [Quality Guidelines](./quality-guidelines.md) | Serialization, dependency, and lint conventions | Complete |
| [Logging Guidelines](./logging-guidelines.md) | Output-free core code and secret redaction | Complete |

## Source References

- `src/zot-core/src/lib.rs`
- `src/zot-core/src/config.rs`
- `src/zot-core/src/error.rs`
- `src/zot-core/src/envelope.rs`
- `src/zot-core/src/model.rs`
- `Cargo.toml`
