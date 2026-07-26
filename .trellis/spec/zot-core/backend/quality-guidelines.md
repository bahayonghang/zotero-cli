# Quality Guidelines

`zot-core` changes have a large blast radius because every crate depends on
its public types. Keep changes small, backward-compatible where possible, and
verified through the workspace gate.

## Required Patterns

- Use workspace dependency inheritance from the root `Cargo.toml`; member
  manifests should use `.workspace = true` for shared dependencies.
- Keep public DTO serialization explicit with serde attributes when payload
  shape matters. Existing examples are `CliEnvelope`, `ChildItem`, and
  `MergeOperation`.
- Prefer `BTreeMap` for deterministic serialized maps in shared models and
  config. Existing examples include `Item::extra`, `LibraryStats`, and
  `AppConfig::profile`.
- Preserve `EnvelopeMeta.api_version`. `docs/agents/limits.md` identifies
  `api_version == 1` as the current JSON contract marker.
- Add focused inline unit tests for parser or helper behavior. Existing
  examples: `parse_library_scope` and default profile tests in `config.rs`.

## Forbidden Patterns

- Workspace lints in root `Cargo.toml` forbid `unsafe_code` and deny
  `dbg_macro`, `todo`, and `unwrap_used`.
- Avoid adding runtime-heavy dependencies to `zot-core`; HTTP, SQLite, async,
  and PDF dependencies belong in downstream crates.
- Avoid public fields whose serialized names are accidental. If consumers rely
  on a shape, document it through serde attributes and tests.

## Code Example

Use serde defaults to keep config additions compatible with existing files:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_format")]
    pub default_format: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self { default_format: default_format(), limit: default_limit() }
    }
}
```

## Scenario: Secret config and atomic persistence

### 1. Scope / Trigger

- Trigger: adding a credential field, changing config paths, or changing `AppConfig::save`.
- Why: derived diagnostics and direct overwrite can leak or destroy the user's write credentials.

### 2. Signatures

```rust
SecretString::expose_secret(&self) -> &str
AppConfig::config_file() -> ZotResult<PathBuf>
AppConfig::state_dir() -> PathBuf
AppConfig::save(&self) -> ZotResult<PathBuf>
```

### 3. Contracts

- Secret fields use serde-transparent `SecretString`; TOML remains a string, while `Debug` is always redacted.
- `config.toml` requires the platform user config directory and never falls back to CWD.
- Non-secret caches/indexes use `state_dir`; they cannot become a config-file fallback.
- Save creates a same-directory temp file, restricts permissions before writing, syncs, then atomically replaces.

### 4. Validation & Error Matrix

| Condition | Result |
|---|---|
| platform config dir missing | `config-dir-unavailable` |
| output format outside `table/json` | `config-value` |
| output limit zero | `config-value` |
| temp write/persist fails | typed `Io`; old target is not pre-truncated |

### 5. Good / Base / Bad Cases

- Good: existing config is atomically replaced and Unix mode remains `0600`.
- Base: absent config returns defaults with `table` and limit 50.
- Bad: derive `Debug` over raw `String` secrets or call `fs::write` on the final path.

### 6. Tests Required

- Canary is absent from `SecretString`, config, and context Debug while TOML round-trips it.
- Unicode redaction asserts character boundaries.
- Atomic overwrite asserts final contents, no temp residue, and Unix `0600`.
- A `None` platform directory asserts `config-dir-unavailable`.

### 7. Wrong vs Correct

Wrong:

```rust
#[derive(Debug)]
struct Config { api_key: String }
std::fs::write(AppConfig::config_file(), encoded)?;
```

Correct:

```rust
#[derive(Debug)]
struct Config { api_key: SecretString }
config.save()?; // same-directory temp + sync + atomic persist
```

## Testing Requirements

- Run `cargo test -p zot-core` for focused changes.
- Run `just ci` before finishing cross-crate or public contract changes.
- When changing envelope fields, also inspect `src/zot-cli/src/format.rs`
  tests because the CLI owns the emitted JSON.

## Review Checklist

- Does the change keep `zot-core` free of CLI, SQLite, HTTP, and PDF runtime
  concerns?
- Are new public types re-exported from `lib.rs` only if downstream crates need
  them?
- Are error codes, serde tags, and env override names stable and documented?
- Are secrets redacted before any payload or diagnostic can expose them?
