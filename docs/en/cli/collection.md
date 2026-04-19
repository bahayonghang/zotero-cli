# collection command

`collection` is used to inspect, search, and maintain real Zotero collections.

## Subcommands

```bash
zot --json collection list
zot --json collection get COLL001
zot --json collection subcollections COLL001
zot --json collection items COLL001
zot --json collection item-count COLL001
zot --json collection tags COLL001
zot --json collection search Transform --limit 20
zot --json collection create "New Project"
zot --json collection rename COLL001 "Renamed Project"
zot --json collection delete COLL001
zot --json collection add-item COLL001 ATTN001
zot --json collection remove-item COLL001 ATTN001
```

## When to use collection

- You are organizing the actual Zotero library structure
- You need to place items into a real collection
- You want subcollections, item counts, or collection-level tags
- You explicitly want to modify remote Zotero collections

## Difference from workspace

- `collection`: changes a real Zotero collection
- `workspace`: maintains a local reading or retrieval workspace without directly changing Zotero collections

If you want a long-lived topic set for reading and query, prefer [workspace](/en/cli/workspace).

## Deletion warning

`collection delete` is destructive. Only run it when the user explicitly requests deletion.
