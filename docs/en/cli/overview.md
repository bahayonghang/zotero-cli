# CLI Overview

## Global flags

`zot` supports these global flags:

| Flag | Meaning |
| --- | --- |
| `--json` | Return the standard JSON envelope for scripts and agents |
| `--profile <name>` | Select a config profile |
| `--library <scope>` | Select library scope: `user` or `group:<id>` |
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
2. Confirm credentials before writes
3. Prefer `--json` for automation
4. Stick to one invocation path per session: `zot ...` or `cargo run -q -p zot-cli -- ...`

## Common starting commands

```bash
zot --json doctor
zot --json library search "attention"
zot --json item get ATTN001
zot --json workspace new llm-safety --description "LLM safety papers"
zot --json sync update-status --apply --limit 20
```

## Command guides

- [library](/en/cli/library)
- [item](/en/cli/item)
- [collection](/en/cli/collection)
- [workspace](/en/cli/workspace)
- [sync / mcp](/en/cli/sync-mcp)
- [Troubleshooting](/en/cli/troubleshooting)
