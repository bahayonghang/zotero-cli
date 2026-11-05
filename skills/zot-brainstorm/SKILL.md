---
name: zot-brainstorm
description: 当用户要基于已有 Zotero collection、workspace 或明确 item key，做研究 brainstorming、缺陷/局限/gap analysis、创新方向或下一步选题，并生成可追溯的 Markdown/HTML 报告时使用，即使用户没说 zot-brainstorm。不要用于普通库内检索、单篇摘要、标签/笔记/分类维护、引用格式教学、脱离 Zotero 的网页找论文，或未经单独授权写入 Zotero/Obsidian；这些分别交给 zot 或停止。 (v1.1.0)
---

# zot-brainstorm

Turn a real Zotero source set into a grounded research brainstorm. Every major finding, gap, and proposed direction must trace to Zotero item keys or citations. Use `zot` for every Zotero read; when the exact read route is unclear, load `../zot/SKILL.md`.

## Scope

Trigger for brainstorming, limitation or gap analysis, innovation directions, and next-step ideas grounded in one or more Zotero collections, workspaces, or explicit item keys.

Do not trigger for ordinary lookup, single-paper summary, library maintenance, citation-format teaching, non-Zotero PDF review, or generic web search. Do not use `zot mcp serve`.

## Operating Rules

- Keep Zotero read-only. Local `report.md` and `report.html` are allowed. Zotero notes, collections, saved searches, Obsidian, or publishing need a separate request and the `zot` safety gates.
- Use one invocation path: prefer `zot --json ...`; in this repository only, use `cargo run -q -p zot-cli -- --json ...` when `zot` is unavailable.
- Run `doctor` before source-scale extraction, PDF or annotation work, workspace indexing or query, or any optional write path.
- Do not read or write `zotero.sqlite`, and do not invent commands such as `zot brainstorm`, `search-in`, `expand`, or `s2`.
- Do not write fulltext-backed claims from metadata or abstracts. Follow the user's prompt language for prose and labels; preserve keys, DOIs, titles, venue names, and source terminology.
- Resolve collection, workspace, and item-key inputs with [references/sources.md](references/sources.md) before synthesis.

## Analysis Sequence

1. Source coverage: counts, years, item types, venues, evidence levels, duplicates, and missing evidence.
2. Theme map: problem, method, dataset or domain, evaluation style, and contribution type.
3. Consensus map: what the selected references appear to agree on.
4. Conflict map: disagreements, competing goals, and exposed assumptions.
5. Defect analysis: weak evaluation, dataset, reproducibility, engineering, theory, and evidence gaps.
6. Brainstorm opportunity areas only after the defect analysis.
7. Rank ideas by novelty, feasibility, evidence support, expected contribution, and the user's goal.
8. Link each major gap and idea to item keys or citations. Label useful but ungrounded ideas as speculative.

Evidence levels are `fulltext`, `metadata+abstract`, `annotations`, `notes`, and `missing`. Their exact limits and deeper commands are in [references/sources.md](references/sources.md).

## Report Output

Default sections are Source Metadata, Coverage Summary, Evidence Quality, Theme Map, Consensus and Conflicts, Defect / Limitation Matrix, Innovation Directions, Ranked Next Steps, Traceability Table, References, and Unresolved Evidence Gaps.

Use `templates/report.md` for content and `templates/report.html` for a saved or visual report. When the user gives no path, write both files under `zot-brainstorm-reports/<yyyy-mm-dd>-<topic-slug>/`. Generate only one format when explicitly requested. The final response names each saved path, the evidence basis, unresolved gaps, and whether Zotero stayed read-only.

HTML is self-contained and offline: no CDN, remote assets, analytics, fetch, or XHR. Read [references/report.md](references/report.md) before writing or adapting the report.

## Failure And Fallback

- Ambiguous sources: show plausible candidates and ask the smallest question. Do not guess.
- A requested source is missing or an explicit item key does not resolve: stop before synthesis unless the user explicitly accepts the remaining sources.
- No PDF backend: continue with metadata, abstracts, notes, and annotations, and mark fulltext claims unavailable.
- Embeddings are unavailable: do not make semantic-only claims; use `workspace query --mode bm25` only when query is still needed.
- A large corpus: summarize coverage first and propose a scoped pass by collection, workspace, theme, year, or query.
- Command-specific resolution and degraded-report behavior are in [references/sources.md](references/sources.md) and [references/report.md](references/report.md).
