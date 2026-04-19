# library command

`library` is the default local read-first surface. It handles “search first, narrow down, then move to item or workspace.”

## Subcommands

- `library search <query>`
- `library list`
- `library recent`
- `library stats`
- `library citekey <citekey>`
- `library tags`
- `library libraries`
- `library feeds`
- `library feed-items <library-id>`
- `library semantic-search <query>`
- `library semantic-index`
- `library semantic-status`
- `library duplicates`
- `library duplicates-merge`
- `library saved-search list`
- `library saved-search create`
- `library saved-search delete`

## search

`library search` supports broad search plus structured filters.

Common examples:

```bash
zot --json library search "transformer attention" --limit 10
zot --json library search "reward hacking" --collection COLL001 --type preprint --limit 20
zot --json library search "attention" --tag attention --creator Vaswani --year 2017
zot --json library search "alignment" --sort date-added --direction desc
```

Available options:

- `--collection <key>`
- `--type <item-type>`
- `--tag <tag>`
- `--creator <name>`
- `--year <yyyy or prefix>`
- `--sort <date-added|date-modified|title|creator>`
- `--direction <asc|desc>`
- `--limit`
- `--offset`

## recent

`library recent` now supports two modes:

```bash
zot --json library recent --count 10
zot --json library recent 2026-04-01 --limit 20
```

Notes:

- `--count <n>` means the latest N library items, returned by `dateAdded desc`
- `<YYYY-MM-DD> --limit <n>` means items since a date boundary
- without arguments, it defaults to `library recent --count 10`

## citation key, tags, libraries, and feeds

```bash
zot --json library citekey Smith2024
zot --json library tags
zot --json library libraries
zot --json library feeds
zot --json library feed-items 3 --limit 20
```

Notes:

- `citekey` uses local Extra-field fallback first and strengthens via Better BibTeX when available
- `library libraries` can enumerate user, group, and feed library summaries together
- feeds are explicit `library feeds` / `feed-items` flows, not a `--library` scope switch

## semantic index / search / status

```bash
zot --json library semantic-status
zot --json library semantic-index --fulltext
zot --json library semantic-index --collection COLL001 --force-rebuild
zot --json library semantic-search "mechanistic interpretability" --mode hybrid --limit 10
```

Supported modes:

- `bm25`
- `semantic`
- `hybrid`

Notes:

- the library-level semantic index is stored in a local sidecar SQLite file
- it reuses the same index implementation as workspace retrieval, but not the same file
- do not assume `semantic` or `hybrid` is meaningful when embeddings are not configured
- `semantic-index` uses **replace-style incremental indexing** by default: without `--force-rebuild`, it rebuilds only the selected items and removes keys that no longer exist in the library
- `--force-rebuild` clears the entire index file before writing; reserve it for real rebuilds (for example, after changing the embedding model)

## duplicates and merge

```bash
zot --json library duplicates --method both --limit 50
zot --json library duplicates --method title
zot --json library duplicates --method doi

zot --json library duplicates-merge --keeper KEEP001 --duplicate DUPE001 --duplicate DUPE002
zot --json library duplicates-merge --keeper KEEP001 --duplicate DUPE001 --duplicate DUPE002 --confirm
```

`duplicates-merge` is dry-run by default. Only `--confirm` performs the actual merge:

- fill keeper metadata fields that are currently empty
- merge tags
- preserve or add collections
- re-parent child items
- skip obviously duplicate attachments when possible
- move duplicate items to Trash

If you already have two explicit item keys rather than a duplicate-candidate set, switch to `item merge` on the [item](/en/cli/item) page.

## saved search

```bash
zot --json library saved-search list
zot --json library saved-search create --name "Recent RL" --conditions conditions.json
zot --json library saved-search delete SRCH0001
```

Notes:

- `saved-search list` returns saved-search metadata and conditions
- `saved-search create` accepts `--conditions` as either a JSON string or a JSON file path
- `saved-search delete` removes the saved search itself, not the items
- Zotero Web API does not currently return saved-search results directly

## Recommended flow

Typical sequence:

1. `library search` or `library citekey`
2. `item get`
3. `item cite` / `item export` / `item pdf` / `item children`

If you are building a long-lived topic set instead of working on one item, move to [workspace](/en/cli/workspace).
