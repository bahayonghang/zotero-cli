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
- `library dedupe`
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
zot --json --write-backend desktop library duplicates-merge --keeper KEEP001 --duplicate DUPE001 --duplicate DUPE002 --confirm
zot --json --write-backend web library duplicates-merge --keeper KEEP001 --duplicate DUPE001 --duplicate DUPE002 --confirm
```

`duplicates-merge` is dry-run by default. Only `--confirm` performs the actual merge:

- fill keeper metadata fields that are currently empty
- merge tags
- preserve or add collections
- re-parent child items
- skip obviously duplicate attachments when possible
- add a `dc:replaces` relation on the keeper for every merged item, so citations already inserted in Word / LibreOffice keep resolving
- move duplicate items to Trash (recoverable; nothing is deleted permanently)

Notes:

- duplicate detection skips items already in Trash
- items of different types can be merged; the keeper keeps its own type, and only fields valid for that type are filled
- source fields the keeper's type does not support are skipped and reported as `skipped_incompatible_fields` (field + source key) in both preview and applied output
- the `dc:replaces` URIs appear as `relations_to_add` in the same output
- without an override the effective profile's `write_backend` is used; preview and confirm stay on one backend, with no automatic fallback
- desktop uses Zotero's native transaction; the Web backend retains its existing multi-request write semantics

If you already have two explicit item keys rather than a duplicate-candidate set, switch to `item merge` on the [item](/en/cli/item) page. To clean the whole library in one pass, use `library dedupe` below.

## dedupe

`library dedupe` is the batch cleanup entry: detect duplicate groups, pick one keeper per group automatically, and emit a cleanup plan for the whole library or one collection.

```bash
zot --json library dedupe
zot --json library dedupe --method doi --limit 100
zot --json library dedupe --collection COLL001
zot --json --write-backend desktop library dedupe --collection COLL001 --confirm
zot --json --write-backend web library dedupe --collection COLL001 --confirm
```

Available options:

- `--method <both|doi|title>` (default `both`)
- `--collection <key>`
- `--limit <n>` (default 50)
- `--confirm`
- `--include-low-confidence`

Without `--confirm` the command is a pure local dry-run: no network access, no write credentials required. The plan JSON contains `groups[]`, `total_groups`, and `confirm_required`; each group carries:

- `match_type`: `doi`, `title`, or a combined value such as `doi+title` — detection groups sharing an item are merged into one component, so every item appears at most once in the plan
- `confidence`: `normal` or `low`; `low` groups add a `confidence_note` (year spread > 1, or differing DOIs inside the group) and deserve a manual look before confirming
- `keeper`: the surviving item (`key`, `item_type`, `title`)
- `reason`: why the keeper won — type priority first (journalArticle = conferencePaper > book / bookSection > thesis > report > preprint > document > others), then tie-breaks on non-empty metadata fields, local attachment count, earlier `dateAdded`, and finally key order
- `absorb`: the items merged into the keeper and moved to Trash

`--confirm` applies the plan group by group through the same selected backend as `duplicates-merge`, including `dc:replaces` and cross-type field safety. One failed group does not abort the rest. By default only normal-confidence groups run; low-confidence groups enter `skipped_low_confidence` and are never sent to the writer. The report includes `applied`, `failed`, `skipped_low_confidence`, `total_groups`, `eligible_groups`, `applied_groups`, `failed_groups`, and `skipped_low_confidence_groups`.

Notes:

- groups may mix item types (for example preprint + conferencePaper); the keeper keeps its own type
- review `confidence: "low"` groups, or start with a single `--collection`, before a whole-library `--confirm`
- ordinary confirmation does not authorize low-confidence groups. Use `--include-low-confidence` only after showing those groups separately and obtaining explicit risk authorization
- desktop requires Zotero running, an installed and paired bridge, and an editable library; desktop failures do not consult Web credentials or fall back
- to hand-pick the keeper of a single group, stay with `library duplicates-merge`

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
