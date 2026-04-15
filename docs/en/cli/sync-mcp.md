# sync / mcp

## sync update-status

`sync update-status` checks whether a preprint has a published version.

Examples:

```bash
zot --json sync update-status ATTN001
zot --json sync update-status --collection COLL001 --limit 20
zot --json sync update-status --apply --limit 20
```

### When to use `--apply`

- If you only want analysis, do not use `--apply`
- If the user explicitly wants the status written back to Zotero, use `--apply`

`--apply` mutates the library and should be treated like any other write action.

## mcp serve

`zot mcp serve` currently exists only as a reserved command surface and returns an unsupported status.

Practical takeaway:

- document that it exists
- do not build workflows around it yet
- use the CLI directly for actual work
