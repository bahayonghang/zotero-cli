# zot-cli Backend Guidelines

`zot-cli` is the binary crate for `zot`. It owns clap surfaces, command
dispatch, context construction, JSON envelope output, human-readable output,
doctor diagnostics, and orchestration between `zot-core`, `zot-local`, and
`zot-remote`.

## Pre-Development Checklist

- Read [Directory Structure](./directory-structure.md) before adding or moving
  commands.
- Read [Database Guidelines](./database-guidelines.md) before opening local
  libraries, workspaces, or remote clients from command handlers.
- Read [Error Handling](./error-handling.md) before adding validation or
  changing JSON error output.
- Read [Quality Guidelines](./quality-guidelines.md) before changing command
  surfaces, tests, or workspace dependency manifests.
- Read [Merge & Dedupe Engine](./merge-dedupe.md) before touching `item merge`,
  `library duplicates-merge`, or `library dedupe` behavior.
- Read [Logging Guidelines](./logging-guidelines.md) before adding output.

## Guidelines Index

| Guide                                           | Description                                                                          | Status   |
| ----------------------------------------------- | ------------------------------------------------------------------------------------ | -------- |
| [Directory Structure](./directory-structure.md) | Binary entrypoint, clap args, command modules, and utilities                         | Complete |
| [Database Guidelines](./database-guidelines.md) | Context-mediated local/remote access boundaries                                      | Complete |
| [Error Handling](./error-handling.md)           | `anyhow` boundary, `ZotError`, and envelope errors                                   | Complete |
| [Quality Guidelines](./quality-guidelines.md)   | CLI parse tests, JSON contract tests, and workspace manifest guard                   | Complete |
| [Merge & Dedupe Engine](./merge-dedupe.md)      | Cross-type merge safety, dc:replaces citation protection, dedupe planning invariants | Complete |
| [Logging Guidelines](./logging-guidelines.md)   | Human vs JSON output rules                                                           | Complete |

## Source References

- `src/zot-cli/src/main.rs`
- `src/zot-cli/src/cli.rs`
- `src/zot-cli/src/cli/args.rs`
- `src/zot-cli/src/context.rs`
- `src/zot-cli/src/format.rs`
- `src/zot-cli/src/commands/`
- `src/zot-cli/tests/workspace_version_guard.rs`
- `skills/zot-skills/SKILL.md`
