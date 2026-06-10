# Database Guidelines

`zot-core` does not own a database. It owns configuration and serialized
contracts that other crates persist or emit.

## Persistence Boundary

- Configuration file path logic lives in `AppConfig::config_dir()` and
  `AppConfig::config_file()` in `src/zot-core/src/config.rs`.
- TOML loading and saving in `AppConfig::{load_raw, load, save}` is the only
  persistence-like code in this crate.
- Local Zotero SQLite reads belong in `zot-local`.
- Library mutations through the Zotero Web API belong in `zot-remote`.
- CLI commands decide when to call those crates; `zot-core` should not open a
  connection, spawn tasks, or reach the network.

## Config Patterns

- Use serde defaults for backward-compatible config fields. Existing examples
  include `OutputConfig::limit`, `ExportConfig::default_style`, and
  `EmbeddingConfig` defaults in `config.rs`.
- Apply profile materialization before environment overrides, as
  `AppConfig::materialize_profile` already does.
- Keep supported env overrides centralized in `config.rs`. Current supported
  names include `ZOT_DATA_DIR`, `ZOT_LIBRARY_ID`, `ZOT_API_KEY`,
  `ZOT_EMBEDDING_URL`, `ZOT_EMBEDDING_KEY`, `ZOT_EMBEDDING_MODEL`,
  `SEMANTIC_SCHOLAR_API_KEY`, and `S2_API_KEY`.

## Code Example

Configuration file writes map filesystem failures into `ZotError::Io` with the
affected path:

```rust
std::fs::write(&path, encoded).map_err(|source| ZotError::Io {
    path: path.clone(),
    source,
})?;
```

Use the same path-preserving style for future config filesystem operations.

## Avoid

- Do not use `rusqlite`, migrations, or transactions in `zot-core`.
- Do not mutate `zotero.sqlite` from any crate. For this crate specifically,
  even read-only access is out of scope.
- Do not add ad hoc config file paths outside `AppConfig::config_dir()` and
  `AppConfig::config_file()`.
