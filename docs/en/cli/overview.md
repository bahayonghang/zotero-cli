# CLI Overview

## Global flags

`zot` supports these global flags:

| Flag | Meaning |
| --- | --- |
| `--json` | Return the standard JSON envelope for scripts and agents |
| `--profile <name>` | Select a config profile |
| `--library <scope>` | Select library scope; only `user` or `group:<id>` is supported |
| `--verbose` | Enable more verbose logging |

## Top-level commands

The current top-level commands come from `src/zot-cli/src/main.rs`:

- `doctor`
- `library`
- `item`
- `collection`
- `workspace`
- `sync`
- `mcp`

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
zot --json library search "attention" --tag transformer --creator Vaswani --year 2017
zot --json library citekey Smith2024
zot --json library semantic-status
zot --json item get ATTN001
zot --json item children ATTN001
zot --json collection search Transform
zot --json workspace query llm-safety "What are the main failure modes?" --mode hybrid --limit 5
```

## Command responsibilities

- `library`: default read-first surface for search, enumeration, semantic flows, feeds, and duplicates
- `item`: single-item inspection plus most write actions, annotations, and Scite
- `collection`: maintenance of real Zotero collections
- `workspace`: local reading workspaces
- `sync`: preprint publication-status checks
- `mcp`: currently reserved, not a usable workflow

## Command guides

- [library](/en/cli/library)
- [item](/en/cli/item)
- [collection](/en/cli/collection)
- [workspace](/en/cli/workspace)
- [sync / mcp](/en/cli/sync-mcp)
- [Troubleshooting](/en/cli/troubleshooting)
