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
- `library organize plan`
- `library organize apply`

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

Notes:

- A non-empty `library search` JSON envelope sets `meta.fulltext_index` to
  `legacy-tables`, `fts5-sidecar`, or `unavailable`. `library list` and empty
  queries omit the field.
- From Zotero userdata ≥127, attachment full text lives in
  `data_dir/fulltext.sqlite`. Search must still succeed when the main-DB word
  tables are gone; `unavailable` means this query did not search attachment
  content.

## list, stats, and tags

```bash
zot --json library list --limit 50 --offset 0
zot --json library list --collection "Machine Learning"
zot --json library stats
zot --json library tags
```

These three read the running Zotero when local communication is available. Pass `--running-zotero` when the data directory is not proved to be that library. Success sets `meta.read_source` to `desktop` or `desktop-unbound` and does not print a human source line.

`library list` excludes attachments, notes, and annotations. It fetches the filtered set, sorts by item key descending locally, then applies `--offset` / `--limit`. It does not send `sort=key`. `--collection` accepts a key or a unique name; duplicate names return `collection-ambiguous`. `--include-trashed` and `meta.trash_policy` stay as they are.

`library stats` counts `top_tags` on bibliographic items only. `pdf_attachments` is the number of distinct bibliographic parents that have an `application/pdf`. `notes` is the number of note rows under bibliographic parents. `library tags` counts every item tag, including notes, attachments, annotations, and trash.

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
- add a `dc:replaces` relation on the keeper for every merged item, so citations already inserted in Word / LibreOffice keep resolving
- move duplicate items to Trash (recoverable; nothing is deleted permanently)

Notes:

- duplicate detection skips items already in Trash
- items of different types can be merged; the keeper keeps its own type, and only fields valid for that type are filled
- source fields the keeper's type does not support are skipped and reported as `skipped_incompatible_fields` (field + source key) in both preview and applied output
- the `dc:replaces` URIs appear as `relations_to_add` in the same output
- preview and confirm both use the Zotero Web API; configure `library_id` and `api_key` before confirmation
- the Web API retains its multi-request, non-transactional write semantics

If you already have two explicit item keys rather than a duplicate-candidate set, switch to `item merge` on the [item](/en/cli/item) page. To clean the whole library in one pass, use `library dedupe` below.

## dedupe

`library dedupe` is the batch cleanup entry: detect duplicate groups, pick one keeper per group automatically, and emit a cleanup plan for the whole library or one collection.

```bash
zot --json library dedupe
zot --json library dedupe --method doi --limit 100
zot --json library dedupe --collection COLL001
zot --json library dedupe --collection COLL001 --confirm
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

## organize

`library organize` accepts only `--write-backend local`. It does not replace `item tag batch` and does not create collections inside the batch.

```bash
zot --json library organize plan --file requests.json --output plan.json --write-backend local
zot --json library organize apply --plan plan.json --confirm --expected-plan-sha256 <sha256> --report report.json --write-backend local
```

Request v1 contains only `schema_version` and `items`. Each item may include `item_key`, `add_tags`, `remove_tags`, `add_collections`, `remove_collections`, `move_collection`, and a display-only `reason`. Unknown fields, duplicate items, contradictory actions, and a move combined with other membership actions are rejected before any write.

Planning reads the Local API. It does not open SQLite, authorize, or PATCH. SHA-256 covers the exact plan-file bytes and is returned in the command JSON, not embedded in the file. A hash mismatch, edited file, invalid schema, or after value that does not match a fresh calculation from `original_request` is rejected before any write.

The default ceiling is 50 changed items, counted once per item. Over-limit means zero writes. plan and apply must use the same explicit ceiling and be reviewed again. Each item gets at most one conditional PATCH. The first failed, conflict, unknown, or verification-failed item stops the batch; later items are not-attempted. There is no automatic rollback or blind retry. Only an explicit 401 may reauthorize once.

Final aggregate states are applied, unchanged, partial, failed, and unknown; a running disk report uses in-progress. Exit 0 requires every item to be applied or unchanged and the report to be saved. Other results exit non-zero, while stdout remains one JSON envelope containing the full report. After a post-submission save failure, stdout retains known outcomes with a report error, while disk may retain in-flight. An in-flight item after process interruption requires review: it proves neither success nor failure and must not be resumed automatically.

Apply recalculates actions, differences, and counts, rejecting forged changed flags or target sets. All targets pass preflight, then each item version (including no-ops) and referenced collection name/version is checked again. Collections are also checked after authorization waits. These collection checks and the item PATCH are still not a cross-object atomic transaction.

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
