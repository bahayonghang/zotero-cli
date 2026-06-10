# Reference-grounded brainstorming skill

## Goal

Create a new skill beside `skills/zot` that helps an agent brainstorm research directions from real Zotero references. The user can select one or more Zotero collections, workspaces, or explicit item keys, then the agent should gather the actual library items, summarize the evidence base, identify limitations or unresolved gaps, and propose next-step innovation points with traceable references.

## User Value

- Turns existing Zotero collections, workspaces, or ad hoc item-key sets into a grounded research ideation surface.
- Reduces ungrounded literature brainstorming by tying every major claim, gap, and idea to real Zotero items.
- Gives the user a repeatable workflow for moving from "papers I already collected" to "what should I try next?"

## Confirmed Facts

- The only current skill directory under `skills/` is `skills/zot`.
- `skills/zot/SKILL.md` is a Zotero operator workflow contract; it routes natural-language Zotero tasks to the Rust `zot` CLI and does not currently define a dedicated brainstorming / innovation workflow.
- The existing `zot` skill already covers collection discovery and item retrieval via `collection list`, `collection search`, `collection items`, `collection get`, `collection item-count`, and `collection tags`.
- The existing runtime supports long-lived local workspaces via `workspace new`, `workspace list`, `workspace show`, `workspace import --collection`, `workspace index`, `workspace search`, `workspace query`, and `workspace export`.
- The existing runtime supports evidence extraction from item metadata, citations, children, notes, annotations, PDF text, outlines, and BibTeX exports.
- `docs/skills/examples.md` already demonstrates a Zotero collection to review workflow: run `doctor`, locate a collection, fetch all collection items, produce a collection summary, then write a review while marking evidence limits.
- `docs/skills/examples/review-llm-in-time-series.md` already includes useful output patterns for this task: source metadata, coverage summary, method evolution, key findings, comparison matrix, convergence/conflict table, research gaps, traceability table, and references.
- `TODO.md` contains a pending `zot-brainstorm` item, which supports using `skills/zot-brainstorm` as the new skill folder name.
- `docs/agents/limits.md` says semantic search and workspace query are O(N) scans over indexed chunks and recommends smaller workspaces or collection filtering for larger libraries.
- There is no need to implement a new Rust CLI command for the MVP; the new skill can orchestrate existing `zot` commands and agent-side synthesis.
- The user chose a read-only report MVP: reports may be saved locally as Markdown, with a reusable Markdown template.
- The user also wants an HTML report template so agents can create a clearer and more visually organized browser-viewable report.
- The user chose Markdown plus HTML as the default saved report output.
- The user chose Zotero collections, workspaces, and explicit item keys as first-class MVP inputs.
- The user chose "follow the user's prompt language" as the generated report language rule.
- The repository does not currently contain an HTML report template for this workflow; the closest existing pattern is the Chinese Markdown review example under `docs/skills/examples/`.
- The HTML report maps best to an Evidence Dossier artifact shape: source-backed synthesis with evidence vs inference separation, source tables, confidence labels, uncertainty, and recommendations.

## Requirements

- Create a new sibling skill directory, planned as `skills/zot-brainstorm`.
- Add a `SKILL.md` with frontmatter name and description that triggers for research brainstorming grounded in Zotero collections, workspaces, or explicit item keys.
- Include a reusable Markdown report template in the skill package.
- Include a reusable self-contained HTML report template in the skill package.
- Base the workflow on `skills/zot`, but keep the new skill focused on synthesis and ideation rather than general Zotero command routing.
- Require the agent to run `doctor` before source-scale evidence extraction, PDF/fulltext work, workspace indexing/querying, or any optional write/persistence path.
- Support selecting one or more real Zotero collections, workspaces, or explicit item keys as first-class MVP inputs.
- Resolve collection names/keys through the existing collection read surface before analysis.
- Resolve workspace names through the existing workspace read surface before analysis, starting from `workspace show <name>` and using `workspace list` when discovery is needed.
- Resolve explicit item keys through `item get <item-key>` before analysis; stop with a clear missing-key report when requested keys cannot be resolved.
- Allow mixed source input sets, deduplicate by item key across collections/workspaces/item-key lists, and preserve source membership for traceability.
- Build a source corpus from real Zotero items and report coverage: collection names/keys, workspace names, explicit item-key inputs, item count, item types, year distribution, evidence levels, duplicate-title warnings, and missing evidence.
- Report source coverage separately by collection, workspace, and explicit item-key source.
- Use a clear evidence priority: fulltext when available, then metadata/abstract, then annotations, then existing notes. Strong claims based only on weaker evidence must be marked as uncertain.
- Produce structured brainstorming output:
  - source metadata and coverage summary
  - paper/theme map
  - current consensus
  - conflicts and weak assumptions
  - defect / limitation analysis
  - opportunity and innovation directions
  - ranked next-step ideas
  - traceability table linking claims and ideas back to item keys or citations
- Save reports locally when requested. If the user does not provide an output path, the skill should choose a predictable local report directory and report the exact paths.
- Saved reports should generate both `report.md` and `report.html` by default.
- Generated report content should follow the user's prompt language by default. Chinese prompts should produce Chinese reports, English prompts should produce English reports, and mixed-language prompts should follow the user's dominant language unless the user explicitly asks otherwise.
- Keep the workflow read-only with respect to Zotero by default. Creating local Markdown/HTML files is allowed; Zotero writes, saved searches, collection edits, notes, Obsidian sync, or external publishing must require a separate explicit user request and existing `zot` safety gates.
- The Markdown template must be content-first and copyable into papers, notes, or planning docs.
- The HTML template must be offline and self-contained: no CDN, remote fonts, remote assets, analytics, `fetch`, or automatic network requests.
- The HTML template must include accessible navigation, tables with captions, evidence/confidence labels that do not depend on color alone, and a print-friendly layout.
- The HTML template should support the same sections as the Markdown template, plus optional visual summaries for year/type/evidence-level distributions, defect matrices, and ranked innovation directions.
- Add regression prompts and evals for the new skill, mirroring the existing `skills/zot/test-prompts.json` and `skills/zot/evals/evals.json` style.
- Update user-facing docs only if the skill is meant to be discoverable from repo docs; at minimum update Chinese docs because the existing collection-review example is currently Chinese-only.

## Acceptance Criteria

- [ ] `skills/zot-brainstorm/SKILL.md` exists with a focused trigger description for reference-grounded brainstorming from Zotero collections/workspaces/item keys.
- [ ] `skills/zot-brainstorm/templates/report.md` exists and defines the default local Markdown report structure.
- [ ] `skills/zot-brainstorm/templates/report.html` exists and defines a self-contained offline HTML report structure.
- [ ] The skill explicitly reuses `zot` as the execution layer and does not invent unsupported commands.
- [ ] The skill defines a concrete source-resolution workflow from `doctor` to collection/workspace/item-key resolution, corpus extraction, evidence grading, synthesis, and innovation ranking.
- [ ] The skill tells agents how to handle multiple collections/workspaces/item-key lists, duplicate items, unavailable PDF/fulltext, missing embeddings, and large source sets.
- [ ] The skill requires traceability for every major finding, gap, and proposed innovation idea.
- [ ] The skill explains when to produce Markdown only and when to also create an HTML report.
- [ ] The skill defaults to saving both Markdown and HTML reports and reports exact local output paths for both files.
- [ ] The skill states that generated report content follows the user's prompt language unless the user explicitly requests another language.
- [ ] The HTML template contains no external network dependency and remains useful when opened directly from disk.
- [ ] The skill states that generic web literature search is out of scope unless the user separately asks for external search.
- [ ] Regression prompts cover at least: multi-collection brainstorming, ambiguous collection names, metadata-only evidence limits, workspace-backed analysis, explicit item-key analysis, mixed collection/workspace/item-key source sets, large/broad review handling, saved Markdown/HTML report generation, and write/sync refusal without explicit permission.
- [ ] Eval expectations check for grounded references, evidence limits, traceability tables, local output paths, offline HTML constraints, and no fabricated unsupported `zot` commands.
- [ ] If docs are updated, they link the new skill without changing the existing `zot` runtime contract.

## Out of Scope

- Adding a new Rust CLI command for brainstorming.
- Implementing a full paper ingestion pipeline from external web search.
- Writing generated ideas back into Zotero notes, saved searches, Obsidian, or other vaults by default.
- Building a full HTML report generator, web app, or build step; the MVP provides a fillable template and workflow instructions.
- Reworking `skills/zot` into a general review-writing skill.
- Building a UI or browser-based selector for collections.
- Treating `zot mcp serve` as available; the current project contract says MCP is scaffolded but not implemented.

## Open Questions

- None blocking. Planning is ready for user review before implementation starts.
