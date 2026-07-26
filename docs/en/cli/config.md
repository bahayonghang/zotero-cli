# config command

`config` inspects and updates `~/.config/zot/config.toml`.

This is a runtime reference page, not the primary agent entrypoint.

## Subcommands

```bash
zot --json config show
zot --json config init --library-id 123456 --api-key abcd
zot --json config init --target-profile work --library-id 123456 --api-key abcd --make-default
zot --json config set library-id 123456
zot --json config set api-key abcd --target-profile work
zot --json config profiles list
zot --json config profiles use work
```

## show

```bash
zot --json config show
zot --json --profile work config show
```

Use it to:

- inspect the effective config
- see the default profile
- see which profile the current session selected
- debug Web write credentials, embeddings, or data-dir state

Legacy `desktop_bridge` and `write_backend` entries are ignored and no longer appear in `config show`. `doctor` emits one migration hint without reading or printing the old token.

## init

```bash
zot --json config init --library-id 123456 --api-key abcd
zot --json config init --target-profile work --library-id 123456 --api-key abcd --make-default
```

Notes:

- without `--target-profile`, it writes root config
- with `--target-profile`, it writes a named profile
- `--make-default` also updates the default profile
- if `data-dir` is not provided, the runtime tries to auto-detect the Zotero data directory

## set

```bash
zot --json config set library-id 123456
zot --json config set api-key abcd --target-profile work
zot --json config set embedding-url https://api.example.com/v1/embeddings
```

Supported keys:

- `data-dir`
- `library-id`
- `api-key`
- `semantic-scholar-api-key`
- `embedding-url`
- `embedding-key`
- `embedding-model`
- `output-format`
- `output-limit`
- `export-style`

Notes:

- `embedding-*` is root-only and does not support `--target-profile`
- `output-format` accepts only `table` or `json`; an explicit `--json` always enables JSON
- `output-limit` must be a positive integer
- `output-limit` is the default only for read-result commands without an explicit `--limit`: library queries/lists, item related/deleted/note search, collection search, workspace show/query, annotation list/search, and Scite search/retractions
- indexing, dedupe, tag batch, and sync keep their own workload or write-safety limits

## profiles

```bash
zot --json config profiles list
zot --json config profiles use work
```

Use it to:

- inspect named profiles
- switch the default profile to a named profile

## Recommended use

If you are just trying to do Zotero work through Claude Code or Codex, start from the skills pages.

Drop to `config` only when:

- you need to initialize write credentials
- the default profile is wrong
- doctor reports missing config
- you need to switch profiles before continuing
