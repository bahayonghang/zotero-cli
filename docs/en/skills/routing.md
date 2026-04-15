# Routing

Choose the command family by user intent:

| User intent | Preferred command | Notes |
| --- | --- | --- |
| Find papers, inspect items, check duplicates, list recent items | `library ...` | Local read-only default |
| Read metadata, attachments, PDF, or citations for a specific item | `item ...` | Best for focused single-item work |
| Update title, tags, notes, or attachments | `item ...` | Requires write access |
| Inspect or organize collections | `collection ...` | Mixed read/write |
| Build a topic-based persistent paper set | `workspace ...` | Local workspace, not a direct Zotero collection edit |
| Run RAG or semantic-style query on a workspace | `workspace index/query` | Gracefully falls back if embeddings are unavailable |
| Check whether a preprint has a published version | `sync update-status` | Confirm before `--apply` |

## Quick rule of thumb

- One item or a few items: prefer `library` / `item`
- A topic set: prefer `workspace`
- Any library mutation: confirm write access before `item`, `collection`, or `sync`

## Startup sequence

1. Pick an invocation path: `zot ...` or `cargo run -q -p zot-cli -- ...`
2. Run `doctor` first for new environments, writes, PDFs, workspace issues, and failure reports
3. Keep the invocation path consistent for the session
