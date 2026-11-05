# Source Resolution And Evidence

Resolve the source set before synthesis. Do not invent commands or silently drop a requested source.

## Source Inputs

Treat these as first-class inputs:

- Zotero collection names or keys
- workspace names
- explicit Zotero item keys
- mixed sets combining collections, workspaces, and item keys

Resolution rules:

- Collection key/name:
  - known key: `collection get <key>`
  - ambiguous name: `collection search <name> --limit 20`; ask only if multiple plausible matches remain
  - discovery: `collection list`
  - corpus: `collection items <collection-key>`
- Workspace name:
  - known name: `workspace show <name>`
  - discovery: `workspace list`
  - indexed retrieval only when useful: `workspace index`, then `workspace query`; use `--mode bm25` when embeddings are unavailable
- Explicit item key:
  - resolve each key with `item get <item-key>`
  - use `item children <item-key>` when attachments, notes, or annotations matter
  - stop and report unresolved requested keys before synthesis; do not silently drop them

For mixed inputs, deduplicate by Zotero item key and preserve every source membership, such as collection key/name, workspace name, and explicit item-key request.

## Evidence Workflow

Build a per-item evidence record before writing conclusions:

- item key
- title
- creators
- year
- item type
- venue or publication title
- DOI or URL when available
- abstract and metadata
- source membership
- available notes, annotations, children, PDF/fulltext, outline, or citation exports
- evidence level

Evidence levels:

- `fulltext`: strongest basis for detailed claims
- `metadata+abstract`: adequate for taxonomy, trend mapping, and broad summaries; weak for detailed limitations
- `annotations`: useful user-curated evidence, but partial
- `notes`: useful user-curated interpretation, but not necessarily paper text
- `missing`: count the item, but do not use it for detailed claims

Use deeper commands only when they are justified by the request and environment:

- `item note list <key>` or `item note search <query>`
- `item annotation list --item-key <key>`
- `item fulltext <key>` or `item pdf <key>`
- `item outline <key>`
- `item cite <key>` or `item export <key> --format bibtex`

## Analysis Sequence

Do not jump straight to ideas. Work in this order:

1. Source coverage: counts by source type, collection, workspace, explicit item-key source, year, item type, venue, evidence level, duplicates, and missing evidence.
2. Theme map: group papers by problem, method, dataset/domain, evaluation style, and contribution type.
3. Consensus map: what the selected references appear to agree on.
4. Conflict map: where papers disagree, optimize different goals, or leave assumptions exposed.
5. Defect analysis: identify limitations, weak evaluations, dataset gaps, reproducibility gaps, engineering bottlenecks, theoretical gaps, and evidence gaps.
6. Brainstorming: propose opportunity areas only after the defect analysis.
7. Ranking: score ideas by novelty, feasibility, evidence support, expected contribution, and alignment with the user's stated goal.
8. Traceability: link each major gap and idea to item keys/citations. Label ungrounded but useful ideas as speculative.
