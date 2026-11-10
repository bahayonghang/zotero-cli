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
zot --json collection create "待读" --write-backend local
zot --json collection create "子类" --parent ROOT2345 --write-backend local
zot --json collection rename COLL001 "Renamed Project"
zot --json collection delete COLL001
zot --json collection delete COLL0001 --write-backend local
zot --json collection delete COLL0001 --write-backend local --confirm --expected-server-id <server_id> --expected-version <version> --expected-descendants-sha256 <sha256>
zot --json collection add-item COLL001 ATTN001
zot --json collection remove-item COLL001 ATTN001
```

`list`, `get`, `subcollections`, `search`, `items`, `item-count`, and `tags` read the running Zotero when local communication is available. Pass `--running-zotero` when the data directory is not proved to be that library. Success sets `meta.read_source` to `desktop` or `desktop-unbound` and does not print a human source line. `items`, `item-count`, and `tags` share one direct-member set from `/items/top`; a child note is not a member. Desktop collection trees omit trashed collections (`meta.trashed_collections` is `omitted`), and desktop members omit direct annotations (`meta.direct_annotations` is `omitted`). A closed Zotero still uses the existing SQLite results.

## When to use collection

`create`, `add-item`, and `remove-item` default to the Web API and accept explicit `--write-backend local`. Local creation confirms with `--expected-library-version`, not an item version. See [local content operations](./local-write.md).
Without `--write-backend local`, `delete` still runs immediately on the Web API. An explicit local backend only previews until confirmation supplies the preview Server ID, the collection object version, and the descendant-key SHA-256 together. A local key must be eight uppercase letters or digits. The delete permanently erases that collection and every descendant collection, including trashed descendants omitted by the Zotero 10.0.8 collection list; children are not promoted. Items stay in the library and only lose those memberships. Rename and reparent stay on the Web API.
On Zotero 10+, `move-item ITEMKEY --from SOURCEKEY --to TARGETKEY --write-backend local`
previews a same-library move while preserving other memberships. See
[local content operations](./local-write.md) for confirmation flags.

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
