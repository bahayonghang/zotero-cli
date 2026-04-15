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

`zot mcp serve` currently exists only as a reserved command surface and returns an unsupported result.

Also note that the old reference-MCP connector-style `search` / `fetch` tools are intentionally not exposed as standalone CLI commands. In the Rust CLI they map to workflows such as:

- `library search`
- `library citekey`
- `item get`
- `item pdf` / `item fulltext` / `item children`
- `workspace query`

Practical takeaway:

- document that `mcp` exists
- do not build workflows around `mcp serve`
- use the CLI directly for real work
