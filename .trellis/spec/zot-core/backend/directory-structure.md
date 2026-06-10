# Directory Structure

`zot-core` is a small shared-library crate. Keep it organized around stable
contracts, not runtime workflows.

## Directory Layout

```text
src/zot-core/src/
├── config.rs    # AppConfig, profile materialization, env overrides, paths
├── envelope.rs  # CliEnvelope and EnvelopeMeta for --json output
├── error.rs     # ZotError, ZotResult, ErrorPayload
├── lib.rs       # Public re-exports for downstream crates
└── model.rs     # Shared Zotero, workspace, RAG, Scite, and merge DTOs
```

## Module Ownership

- Put configuration shape, `~/.config/zot/config.toml` path helpers, and
  `LibraryScope` parsing in `config.rs`. Existing examples:
  `AppConfig::load`, `EmbeddingConfig::apply_env_overrides`, and
  `parse_library_scope`.
- Put JSON envelope-only types in `envelope.rs`. `CliEnvelope` and
  `EnvelopeMeta` are consumed by `src/zot-cli/src/format.rs`.
- Put error variants and conversion to public error payloads in `error.rs`.
  Downstream crates should return `ZotResult<T>` instead of inventing crate
  local public error shapes.
- Put shared data-transfer objects in `model.rs`. Keep DTOs here when multiple
  crates need the same serialized shape, such as `Item`, `ChildItem`,
  `SemanticHit`, `SciteItemReport`, and `MergeOperation`.
- Keep `lib.rs` as a re-export surface. It currently exposes the stable names
  downstream crates use, for example `AppConfig`, `CliEnvelope`, `ZotError`,
  and all models via `pub use model::*`.

## Naming Conventions

- Types are Rust `UpperCamelCase`; helper functions are `snake_case`.
- Serialized enums that are command/API payloads use explicit serde tagging.
  `ChildItem` uses `#[serde(tag = "kind", rename_all = "kebab-case")]`;
  `MergeOperation` uses `#[serde(tag = "status", rename_all = "kebab-case")]`.
- Configuration fields mirror the TOML and environment vocabulary already in
  `config.rs`: `data_dir`, `library_id`, `api_key`,
  `semantic_scholar_api_key`, and `embedding`.

## Code Example

`src/zot-core/src/model.rs` keeps child item variants discriminated instead of
using many nullable fields:

```rust
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChildItem {
    Note(ChildNote),
    Attachment(ChildAttachment),
    Annotation(ChildAnnotation),
}
```

Follow that pattern when a JSON consumer must distinguish variants.

## Avoid

- Do not add CLI parsing or printing here; `zot-cli` owns clap and stdout.
- Do not add SQLite access here; `zot-local` owns local database reads and
  sidecar indexes.
- Do not add Zotero Web API or enrichment clients here; `zot-remote` owns HTTP.
- Do not add product behavior that only one crate uses unless it is a shared
  contract type or helper.
