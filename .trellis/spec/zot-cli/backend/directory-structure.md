# Directory Structure

`zot-cli` keeps the executable surface in a thin binary crate. Command modules
should parse arguments, assemble typed options, call `zot-local`/`zot-remote`,
and print through shared formatting helpers.

## Directory Layout

```text
src/zot-cli/src/
├── main.rs            # tokio main, top-level error printing, exit code
├── cli.rs             # top-level Cli, Commands, shared ValueEnum adapters
├── cli/args.rs        # clap Args and subcommand enums
├── context.rs         # AppContext, local_library(), remote(), paths
├── format.rs          # JSON envelope and human-readable output helpers
├── util.rs            # shared CLI helpers: PDF offload, JSON input, pages
└── commands/
    ├── collection.rs
    ├── config.rs
    ├── doctor.rs
    ├── library.rs
    ├── mcp.rs
    ├── sync.rs
    ├── workspace.rs
    └── item/
```

## Module Ownership

- `main.rs` owns `Cli::parse()`, `AppContext` construction via `run`, JSON vs
  human error printing, and process exit code.
- `cli.rs` owns the root command enum and shared value-enum conversions into
  `zot-local` types.
- `cli/args.rs` owns the large command argument tree. Keep new clap argument
  structs here unless a command-local type is not part of the clap surface.
- `context.rs` owns config/profile/library scope materialization and creates
  `LocalLibrary`/`ZoteroRemote` through typed methods.
- `format.rs` owns all shared output helpers and the envelope API version.
- `commands/*` modules own subcommand orchestration. Keep side effects there,
  not in `zot-core`.

## Command Pattern

`commands/mod.rs` is the dispatch table:

```rust
match command {
    Commands::Doctor => doctor::handle(ctx).await,
    Commands::Library { command } => library::handle(ctx, command).await,
    Commands::Item { command } => item::handle(ctx, command).await,
    ...
}
```

Add a new command by updating the clap enum, dispatch, focused handler, and
parse tests in `cli.rs`.

## Avoid

- Do not put long clap argument lists back into `cli.rs`; `cli/args.rs` exists
  to keep the root file small.
- Do not create local or remote clients directly in random helpers; use
  `AppContext::local_library()` and `AppContext::remote()`.
- Do not implement `zot mcp serve` workflows. The command is scaffolded and
  currently returns `mcp-not-implemented`.
