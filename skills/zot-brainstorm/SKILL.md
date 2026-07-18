---
name: zot-brainstorm
description: Use when the user wants Zotero-backed research brainstorming, gap analysis, limitation analysis, innovation directions, or next-step research ideas from existing Zotero collections, workspaces, or explicit item keys. This skill turns real Zotero references into traceable Markdown and HTML reports. Do not use for ordinary Zotero lookup, library maintenance, generic web literature search, citation-format teaching, or paper summaries that are not grounded in Zotero sources. (v1.0.0)
---

# zot-brainstorm

Use this skill to turn a real Zotero source set into a grounded research brainstorming report. The output must be evidence-backed: every major finding, gap, and proposed innovation direction should trace back to Zotero item keys or citations.

This is a synthesis skill. Use `zot` as the execution layer for all Zotero reads, and consult `../zot/SKILL.md` when exact command routing is unclear.

## Scope

Trigger on requests such as:

- brainstorm research directions from one or more Zotero collections
- analyze limitations or defects across a workspace
- propose innovation points from explicit item keys
- create a local Markdown/HTML report from selected Zotero references
- compare themes, evidence strength, gaps, and next-step ideas across a source set

Do not trigger for:

- generic web literature search unless the user explicitly asks to augment Zotero with web sources
- ordinary Zotero lookup, workspace maintenance, saved search, tags, notes, or collection edits
- a single-paper summary that does not ask for brainstorming, gaps, synthesis, or innovation
- non-Zotero PDF review
- any workflow that would require `zot mcp serve`

## Operating Rules

- Keep Zotero read-only by default. Local `report.md` and `report.html` files are allowed; Zotero notes, collections, saved searches, Obsidian sync, vault writes, or external publishing require a separate explicit request and the base `zot` safety gates.
- Use one invocation path for the whole run: prefer `zot --json ...`; in this repository only, fall back to `cargo run -q -p zot-cli -- --json ...` when `zot` is unavailable.
- Run `doctor` before source-scale extraction, PDF/fulltext/annotation work, workspace indexing/querying, or any optional write path.
- Do not read or write `zotero.sqlite` directly.
- Do not invent commands such as `zot brainstorm`, `search-in`, `expand`, `s2`, or `zot mcp serve`.
- Preserve evidence limits. If only metadata/abstract is available, do not write claims that sound fulltext-backed.
- Follow the user's prompt language for generated prose and visible report labels unless the user explicitly asks for another language. Preserve item keys, DOIs, titles, venue names, and source terminology as written.

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

## Report Output

Default report sections:

- Source Metadata
- Coverage Summary
- Evidence Quality
- Theme Map
- Consensus and Conflicts
- Defect / Limitation Matrix
- Innovation Directions
- Ranked Next Steps
- Traceability Table
- References
- Unresolved Evidence Gaps

Use `templates/report.md` as the canonical content template. Use `templates/report.html` when the user asks for a saved report, a browser-viewable report, or a clearer visual report.

When saving and the user does not provide a path, use:

```text
zot-brainstorm-reports/<yyyy-mm-dd>-<topic-slug>/
  report.md
  report.html
```

Saved reports generate both `report.md` and `report.html` by default. Generate only one format when the user explicitly asks for only one.

The final response must include:

- exact local path to each saved report
- evidence basis used: fulltext, metadata/abstract, annotations, notes, or mixed
- unresolved source or evidence gaps
- whether Zotero was kept read-only

## HTML Constraints

HTML reports must be self-contained and offline:

- no CDN
- no remote fonts, images, scripts, or stylesheets
- no analytics
- no `fetch`, XHR, or hidden network calls
- accessible headings and landmarks
- skip link, focus styles, table captions, and `scope` attributes
- visible confidence/status text that does not rely on color alone
- print-friendly CSS
- safe wrapping for mixed Chinese and English text

Prefer dense evidence tables over decorative card sprawl. Small visual summaries may use inline CSS or inline SVG only when the same data is also available as text or a table.

## Failure and Fallback

- If `doctor` reports no PDF backend, proceed with metadata/abstract/notes/annotations and mark fulltext claims unavailable.
- If embeddings are unavailable, avoid semantic-only claims and use workspace query with `--mode bm25` when query is still needed.
- If source inputs are ambiguous, ask the smallest clarifying question after showing the plausible candidates.
- If a requested source is missing, stop before synthesis unless the user explicitly says to continue with the remaining sources.
- If the corpus is large, summarize coverage first and propose a scoped pass by collection, workspace, theme, year, or query.
