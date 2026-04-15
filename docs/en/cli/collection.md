# collection command

`collection` is used to inspect or maintain Zotero collections.

## Subcommands

```bash
zot --json collection list
zot --json collection items COLL001
zot --json collection create "New Project"
zot --json collection rename COLL001 "Renamed Project"
zot --json collection delete COLL001
zot --json collection add-item COLL001 ATTN001
zot --json collection remove-item COLL001 ATTN001
```

## When to use collection

- You are organizing the actual Zotero library structure
- You need to attach items to an existing collection
- You explicitly want to modify remote Zotero collections

## Difference from workspace

- `collection`: changes a real Zotero collection
- `workspace`: maintains a local reading/retrieval workspace without directly changing Zotero collections

If you want a persistent topic set for reading and query, prefer [workspace](/en/cli/workspace).

## Deletion warning

`collection delete` is destructive. Only run it when the user explicitly requests deletion.
