# Routing

Choose the command family by user intent:

| User intent | Preferred command | Notes |
| --- | --- | --- |
| Search by query, tag, creator, year, or collection | `library search` | Default read-first surface |
| Look up an item by citation key | `library citekey` | Strengthens via Better BibTeX when available |
| Inspect tags, libraries, feeds, or feed items | `library tags` / `libraries` / `feeds` / `feed-items` | feeds are not targeted through `--library` |
| Build a semantic index or run semantic search | `library semantic-*` | check doctor and embeddings first |
| Read one item deeply: metadata, PDF, children, outline | `item ...` | Single-item detail surface |
| Add an item by DOI, URL, or file | `item add-doi` / `add-url` / `add-file` | `item create` remains backward compatible |
| Manage notes, tags, annotations, or Scite checks | `item note ...` / `item tag ...` / `item annotation ...` / `item scite ...` | annotation creation has prerequisites |
| Search or maintain collections | `collection ...` | Real Zotero collection read/write |
| Build a long-lived topic set | `workspace ...` | local workspace, not a direct Zotero collection edit |
| Check publication status for preprints | `sync update-status` | confirm before `--apply` |

## Quick rule of thumb

- one item or a few items: prefer `library` / `item`
- citation key, feeds, semantic, annotation, and Scite requests should go straight to the matching advanced subcommands
- a topic set: prefer `workspace`
- any mutation: confirm write access before `item`, `collection`, or `sync`

## Startup sequence

1. pick an invocation path: `zot ...` or `cargo run -q -p zot-cli -- ...`
2. run `doctor` first for new environments, writes, PDF tasks, semantic flows, Better BibTeX, and failure reports
3. keep the invocation path consistent for the session
