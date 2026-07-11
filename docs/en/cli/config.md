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
zot --json config set write-backend desktop
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
- debug the effective `write_backend`, desktop bridge, Web write credentials, embeddings, or data-dir state

`desktop_bridge` exposes only configured state, versions, and a short `connection_id`; it never exposes the long-lived token. Old config without `write_backend` defaults to `web`.

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
zot --json config set write-backend web --target-profile work
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
- `write-backend` (`web` or `desktop`)

Notes:

- `embedding-*` is root-only and does not support `--target-profile`
- `output-limit` must be a positive integer

## desktop bridge

```bash
zot --json bridge setup
zot --json bridge pair PAIR-CODE
zot --json bridge status
zot --json bridge revoke
```

- `setup` only generates the XPI and opens its folder; the user installs it manually and restarts Zotero
- Zotero UI displays the five-minute, single-use pairing code; never put a real code or token in logs, issues, or prompts
- successful pairing sets the current config target's `write_backend` to `desktop`
- the global `--write-backend desktop|web` is a one-call override and is not persisted
- plugin-not-installed, Zotero-stopped, auth, and protocol failures do not fall back to Web
- the first desktop release supports merge/dedupe only; other mutations still require an explicit Web backend and Web credentials

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
- you need to install, pair, or revoke the desktop bridge
- the default profile is wrong
- doctor reports missing config
- you need to switch profiles before continuing
