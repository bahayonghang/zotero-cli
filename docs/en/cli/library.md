# library command

`library` is the default read-only entrypoint for browsing and searching local Zotero data.

## Subcommands

- `library search <query>`
- `library list`
- `library recent <YYYY-MM-DD>`
- `library stats`
- `library duplicates`

## search

Common examples:

```bash
zot --json library search "transformer attention" --limit 10
zot --json library search "reward hacking" --collection COLL001 --limit 20
zot --json library search "alignment" --type journalArticle --sort date-added --direction desc
```

Available options:

- `--collection <key>`
- `--type <item-type>`
- `--sort <date-added|date-modified|title|creator>`
- `--direction <asc|desc>`
- `--limit`
- `--offset`

## list / recent / stats / duplicates

```bash
zot --json library list --limit 20
zot --json library recent 2026-01-01 --limit 20
zot --json library stats
zot --json library duplicates --limit 20
```

## Recommended flow

The usual sequence is:

1. `library search`
2. `item get`
3. `item cite` / `item export` / `item pdf`

If you are organizing a persistent topic set instead of reading one or two items, move to [workspace](/en/cli/workspace).
