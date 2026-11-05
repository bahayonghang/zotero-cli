# Report Contract

Follow the analysis order in SKILL.md. This file owns the saved report contract and degraded-source behavior.

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
