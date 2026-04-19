# Migrating from `ref/zotero-cli`

This page answers one question: if you used `ref/zotero-cli` before, what is the right mental model and command replacement in the Rust `zot` runtime?

## Summary first

| Type | Conclusion |
| --- | --- |
| Already covered | `search`, `get`, `annotations`, `notes`, `collections`, `collection`, `add doi/url`, `tags` |
| Added in this pass | `recent [n]`, explicit `item merge`, `completions <shell>` |
| Intentionally not restored | `--api-base`, flat top-level aliases, compact JSON as the default, connector-style primary commands |

## Command mapping

| `ref/zotero-cli` | Current `zot` |
| --- | --- |
| `search <query>` | `library search <query>` |
| `get <key>` | `item get <key>` |
| `annotations <key>` | `item annotation list --item-key <key>` or `item pdf <key> --annotations` |
| `notes <key>` | `item note list <key>` |
| `collections` | `collection list` |
| `collection <id>` | `collection items <id>` |
| `add doi <doi>` | `item add-doi <doi>` |
| `add url <url>` | `item add-url <url>` |
| `tags` | `library tags` |
| `recent 10` | `library recent --count 10` |
| `merge KEY1 KEY2` | `item merge KEY1 KEY2` |
| `completions powershell` | `completions powershell` |

## What was added in this pass

### Recent N items

Old usage:

```bash
zotero-cli recent 10
```

Current usage:

```bash
zot --json library recent --count 10
```

If what you want is “items since a date”, that still exists as a separate mode:

```bash
zot --json library recent 2026-04-01 --limit 20
```

The two meanings are now explicit:

- `--count` means the latest N items
- `<since> --limit` means date-bounded recent items

### Manual merge

The reference CLI allowed direct merge of any two top-level items. The Rust CLI now exposes that explicitly too:

```bash
zot --json item merge KEEP001 DUPE001
zot --json item merge KEEP001 DUPE001 --confirm
zot --json item merge KEEP001 DUPE001 --keep DUPE001 --confirm
```

It is preview-first by default. Without `--confirm`, nothing is written.

The preview reports:

- which metadata fields will be filled
- which tags and collections will be added
- how many child items will be re-parented
- how many duplicate attachments will be skipped

If you already have duplicate candidates, you can still start from:

```bash
zot --json library duplicates --method both
zot --json library duplicates-merge --keeper KEEP001 --duplicate DUPE001 --confirm
```

`duplicates-merge` and `item merge` now share the same merge rules.

### Completions

The Rust CLI now exposes shell completion generation directly:

```bash
zot completions bash
zot completions zsh
zot completions fish
zot completions powershell
```

## Why the old command style is not being restored

The Rust runtime is not a flat connector wrapper. Its boundaries are deliberate:

- local reads come from `zotero.sqlite` plus attachment storage
- writes go through the Zotero Web API only
- output stays centered on the stable JSON envelope
- the primary mental model is `library` / `item` / `collection` / `workspace` / `sync`

So these legacy traits are intentionally not coming back:

- `--api-base`
- flat top-level aliases like `search` and `get`
- compact JSON as the default response shape
- a second primary command surface built around connector-style `search` / `fetch`

## What this means for agents and the skill

If you installed `zot-skills`, the migrated natural-language routes should now read like this:

- “Show me the last 10 items added to the library” -> `library recent --count`
- “Preview a merge for these two items first” -> `item merge`
- “Find duplicates first, then merge them” -> `library duplicates` / `duplicates-merge`

For the fuller agent-facing routing contract, see:

- [Skills Overview](/en/skills/overview)
- [Routing](/en/skills/routing)
- [CLI Overview](/en/cli/overview)
