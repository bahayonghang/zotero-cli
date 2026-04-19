# CLI Overview

## Global flags

`zot` supports these global flags:

| Flag | Meaning |
| --- | --- |
| `--json` | Return the standard JSON envelope for scripts and agents |
| `--profile <name>` | Select a config profile |
| `--library <scope>` | Select library scope; only `user` or `group:<id>` is supported |

## Top-level commands

The current top-level commands come from `src/zot-cli/src/main.rs`:

- `doctor`
- `config`
- `library`
- `item`
- `collection`
- `workspace`
- `sync`
- `mcp`
- `completions`

## JSON envelope

Success:

```json
{"ok": true, "data": {}, "meta": {}}
```

Failure:

```json
{"ok": false, "error": {"code": "...", "message": "...", "hint": "..."}}
```

## Recommended runtime habits

1. Run `doctor` first in a new environment
2. Confirm credentials and doctor output before writes
3. Prefer `--json` for automation
4. Stick to one invocation path per session: `zot ...` or `cargo run -q -p zot-cli -- ...`
5. Treat feeds as explicit `library feeds` / `library feed-items` flows, not as a global `--library` scope

## Common starting commands

```bash
zot --json doctor
zot --json config show
zot --json library search "attention" --tag transformer --creator Vaswani --year 2017
zot --json library recent --count 10
zot --json library citekey Smith2024
zot --json library semantic-status
zot --json item get ATTN001
zot --json item merge KEEP001 DUPE001
zot --json item download ATCH005
zot --json item children ATTN001
zot --json collection search Transform
zot --json workspace query llm-safety "What are the main failure modes?" --mode hybrid --limit 5
zot completions powershell
```

## Command responsibilities

- `config`: inspect and update runtime config, profiles, and write credentials
- `library`: default read-first surface for search, enumeration, semantic flows, feeds, and duplicates
- `item`: single-item inspection, most write actions, attachment download, annotations, and Scite
- `collection`: maintenance of real Zotero collections plus fine-grained collection reads
- `workspace`: local reading workspaces
- `sync`: preprint publication-status checks
- `mcp`: currently reserved, not a usable workflow
- `completions`: generate shell completions for bash / zsh / fish / powershell

## Migrating from ref\zotero-cli

If you used `ref/zotero-cli` before:

- `recent 10` now maps to `library recent --count 10`
- generic two-item merge now maps to `item merge KEY1 KEY2`
- old flat top-level aliases and `--api-base` are intentionally not returning

For the full migration guide, see [Migrating from ref\zotero-cli](/en/guide/migrating-from-ref-zotero-cli).

## Migrating from ref\zotagent

If you used `ref/zotagent` before:

- `sync` here is not attachment indexing; it is publication-status sync for preprints
- `status` has no single equivalent command; today the real sources are `doctor` + `library semantic-status`
- `search-in`, `metadata`, `read`, and `expand` are not implemented yet
- `s2` and import by `paperId` are also not implemented yet

For the full comparison and completion plan, see [Migrating from ref\zotagent](/en/guide/migrating-from-ref-zotagent).

## Command guides

- [config](/en/cli/config)
- [library](/en/cli/library)
- [item](/en/cli/item)
- [collection](/en/cli/collection)
- [workspace](/en/cli/workspace)
- [sync / mcp](/en/cli/sync-mcp)
- [Troubleshooting](/en/cli/troubleshooting)
