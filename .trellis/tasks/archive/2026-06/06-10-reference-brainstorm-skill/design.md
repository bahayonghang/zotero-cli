# Design: Reference-grounded brainstorming skill

## Architecture and Boundaries

This is a skill-package addition, not a runtime change.

- New package surface: `skills/zot-brainstorm/`
- Existing runtime surface: `zot --json ...`
- Existing base skill: `skills/zot/SKILL.md`
- Planned package files:
  - `skills/zot-brainstorm/SKILL.md`
  - `skills/zot-brainstorm/test-prompts.json`
  - `skills/zot-brainstorm/evals/evals.json`
  - `skills/zot-brainstorm/templates/report.md`
  - `skills/zot-brainstorm/templates/report.html`

The new skill should not duplicate the whole `zot` routing guide. It should reference the existing runtime contract at the concept level and define the higher-level synthesis workflow:

1. select real Zotero collections, workspaces, or explicit item keys
2. extract corpus and evidence
3. grade evidence quality
4. synthesize themes and limitations
5. generate ranked innovation directions with references

## Data Flow

### 1. Environment and invocation path

Use the existing `zot` skill contract:

- Prefer `zot --json ...` when installed.
- Fallback inside this repository: `cargo run -q -p zot-cli -- --json ...`.
- Pick one invocation path and keep it for the whole session.
- Run `doctor` before collection-scale analysis, PDF/fulltext extraction, workspace indexing/querying, or any write path.

### 2. Source selection

First-class MVP inputs:

- one or more Zotero collection names or keys
- one or more workspace names
- one or more explicit Zotero item keys
- mixed source sets combining any of the above

Collection resolution path:

- known key: `zot --json collection get <key>`
- ambiguous name: `zot --json collection search <name> --limit 20`, then ask the user only if multiple plausible matches remain
- broad discovery: `zot --json collection list`

Workspace resolution path:

- known name: `zot --json workspace show <name>`
- discovery: `zot --json workspace list` when the user asks for available workspaces or the requested name cannot be resolved
- use `workspace index` and `workspace query` only when the analysis needs indexed retrieval and `doctor` / embedding prerequisites are satisfied, or fall back to `--mode bm25` when embeddings are unavailable

Explicit item-key resolution path:

- requested key: `zot --json item get <item-key>`
- deeper evidence when needed: `zot --json item children <item-key>`, then notes, annotations, fulltext, outline, or export commands as appropriate
- missing keys: stop and report the unresolved keys clearly before synthesis rather than silently dropping them

Corpus extraction:

- `zot --json collection items <collection-key>` for each selected collection
- `zot --json workspace show <workspace-name>` for each selected workspace, using the workspace membership as the source set
- `zot --json item get <item-key>` for each explicit item key
- deduplicate by item key across collections, workspaces, and item-key lists
- preserve all source memberships for traceability, including collection key/name, workspace name, and explicit item-key request

### 3. Evidence extraction

Minimum per-item record:

- key
- title
- creators
- year
- item type
- venue/publication title
- DOI/URL when available
- abstract/metadata fields returned by `item get` or collection items
- source membership: collection, workspace, and/or explicit item-key source

Deeper evidence when available and useful:

- `item children <key>` for notes, attachments, annotations
- `item note list <key>` or `item note search <query>` when notes are central
- `item annotation list --item-key <key>` for existing annotations
- `item fulltext <key>` or `item pdf <key>` only when `doctor` reports PDF support
- `item outline <key>` for long fulltext documents where structure helps
- `item cite <key>` or `item export <key> --format bibtex` for final references

Evidence grading:

- `fulltext`: strongest textual basis
- `metadata+abstract`: adequate for taxonomy and broad trend mapping, weak for fine-grained claims
- `annotations`: useful user-curated evidence, but partial
- `existing notes`: useful user-curated evidence, but may include interpretation
- `missing`: item can be counted but should not support detailed claims

The skill must mark claims that depend only on `metadata+abstract`, `annotations`, or `notes` when they would otherwise sound stronger than the evidence supports.

### 4. Analysis workflow

The skill should force an intermediate analysis pass before ideation:

1. Coverage summary: counts by source type, collection, workspace, explicit item-key source, year, type, venue, evidence level, duplicate titles.
2. Theme clustering: group items by method/topic/problem/application.
3. Consensus map: what most papers appear to agree on.
4. Conflict map: where papers disagree or optimize different objectives.
5. Defect analysis: limitations, missing evaluations, weak evidence, engineering bottlenecks, theoretical gaps, dataset gaps, reproducibility gaps.
6. Innovation brainstorming: propose ideas only after the defect analysis.
7. Ranking: score ideas by novelty, feasibility, evidence support, and alignment with the user's stated research goal.
8. Traceability: each major gap and idea links to at least one source item key/citation; ungrounded ideas are labeled speculative.

## Output Contract

Recommended default output:

- Source Metadata
- Coverage Summary
- Theme Map
- Consensus and Conflicts
- Limitation / Defect Matrix
- Innovation Directions
- Ranked Next Steps
- Traceability Table
- References

The MVP is read-only with respect to Zotero but can write local report files.

### Markdown report

`templates/report.md` should be the canonical content template. It should stay easy to paste into a note, paper-planning document, or GitHub issue. Recommended sections:

1. Source Metadata
2. Coverage Summary
3. Evidence Quality
4. Theme Map
5. Consensus and Conflicts
6. Defect / Limitation Matrix
7. Innovation Directions
8. Ranked Next Steps
9. Traceability Table
10. References
11. Unresolved Evidence Gaps

The Markdown template should include placeholders for:

- source collections and keys
- workspace names
- explicit item keys
- item counts
- evidence-level counts
- generation date
- invocation path
- report confidence
- local output path

### HTML report

`templates/report.html` should be a self-contained Evidence Dossier template. It is not a web app and should not introduce a build step.

Design decisions:

- Visual direction: dossier / research report.
- Memory hook: evidence rail beside the opening thesis.
- Density strategy: medium-density report; use tables for dense evidence rather than card sprawl.
- Diagram strategy: table-first, with optional inline SVG or structured HTML lanes for evidence-to-claim and innovation roadmaps.

Required HTML properties:

- Single file by default.
- Offline and self-contained.
- No CDN, remote fonts, remote images, remote stylesheets, remote scripts, `fetch`, XHR, analytics, or hidden network calls.
- Semantic structure with `<!doctype html>`, `meta charset`, viewport, one `h1`, `main id="main"`, skip link, and useful landmarks.
- Tables use captions, header scopes, and visible confidence/status text.
- Charts or SVGs include text equivalents or data tables.
- Print-friendly CSS.
- Long mixed Chinese/English text wraps safely.

Recommended HTML sections:

- split hero: research question, bottom-line finding, evidence rail
- quick stats: source count, evidence mix, time span, confidence
- coverage charts: year distribution, item type distribution, evidence-level distribution
- source table: item key/citation, type, year, evidence level, used for
- theme cards or matrix
- defect matrix
- innovation cards ranked by novelty, feasibility, evidence support, and expected contribution
- traceability table linking claim -> evidence -> inference -> proposed next step
- appendix with unresolved evidence gaps

### Saved report paths

If the user gives an output path, use it.

If the user asks to save but gives no path, use a predictable local directory. Recommended default:

```text
zot-brainstorm-reports/<yyyy-mm-dd>-<topic-slug>/
  report.md
  report.html
```

The final user-facing answer must report exact paths and clearly state whether the report is based on fulltext, metadata/abstract, annotations, notes, or mixed evidence.

### Language rule

Generated report content follows the user's prompt language by default.

- Chinese prompt: produce Chinese report prose and Chinese section labels when filling templates.
- English prompt: produce English report prose and English section labels when filling templates.
- Mixed prompt: follow the dominant user-request language unless the user explicitly asks for a different output language.
- Evidence identifiers such as Zotero item keys, DOIs, titles, venue names, and quoted paper terminology remain in their source language.

The Markdown and HTML templates may contain neutral placeholder labels, but the skill instructions should require agents to localize filled report prose and visible section labels to the selected output language.

## Compatibility Notes

- Do not rely on `zot mcp serve`; project docs state it currently returns `mcp-not-implemented`.
- Do not directly read or write `zotero.sqlite`; all local reads go through `zot-local`, and mutations go through the existing Web API paths.
- Do not invent unsupported `zot` commands such as `zot brainstorm`, `search-in`, `expand`, or `s2`.
- Keep generic web search out of scope unless the user explicitly asks to augment beyond Zotero.
- Large collections should prefer collection filtering or workspace scoping because semantic query is an O(N) scan over indexed chunks.

## Trade-offs

### Skill-only MVP vs new CLI command

Recommended: skill-only MVP.

Why: Existing CLI surfaces already expose collection, workspace, item, fulltext, note, annotation, and export data. Brainstorming is mostly agent-side synthesis and prompt discipline.

Trade-off: A future CLI command could standardize corpus summaries, but building it now would slow the MVP and add schema/API decisions before the workflow is validated.

### Collections, workspaces, and item keys as first-class inputs

Decision: collections, workspaces, and explicit item keys are all first-class MVP inputs.

Why: The user may already organize literature in Zotero collections, long-lived local workspaces, or ad hoc item-key sets. Treating all three as source selectors keeps the skill useful without forcing corpus reshaping before brainstorming.

Trade-off: The skill boundary must stay narrow: these inputs should trigger `zot-brainstorm` only when the user asks for synthesis, gap analysis, or innovation directions. Ordinary collection browsing, workspace maintenance, or item inspection should still route to `skills/zot`.

### Read-only local reports vs writing notes/vault pages

Recommended: read-only local reports by default.

Why: The core value is grounded brainstorming, not library mutation. Local Markdown and HTML files preserve the result without requiring Zotero write credentials, Obsidian configuration, or external publishing.

Trade-off: Users may eventually want persistent Obsidian or Zotero-note outputs. That can be added as an explicit opt-in path after the report format stabilizes.

### Markdown-only vs Markdown plus HTML

Decision: generate both Markdown and HTML by default for saved reports.

Why: Markdown is the durable canonical text surface, while HTML makes broad reviews, dense matrices, and visual evidence summaries easier to inspect. Generating both by default gives the user an editable source and a clearer browser-viewable report in the same run.

Trade-off: Always generating both can be overkill for a small collection or quick brainstorm and adds HTML validation cost, but the output is more complete and shareable.

### Fixed output language vs prompt-following language

Decision: follow the user's prompt language by default.

Why: This repository already contains both Chinese and English docs/examples, and the user works in Chinese while research artifacts may need English. Prompt-following keeps quick Chinese ideation natural and English sharing/reporting available without a separate option.

Trade-off: A fixed English-first policy would make long-term research archives more uniform, while Chinese-first would match the current conversation better. Prompt-following is more flexible but requires evals to check both Chinese and English cases.

## Rollback / Operational Considerations

- The implementation is mostly additive. Rollback is deleting `skills/zot-brainstorm/` and reverting docs links.
- If evals prove too broad, narrow the skill description rather than expanding `skills/zot`.
- If users expect this to trigger for ordinary Zotero search, tighten the description so `zot` remains the default operator skill and `zot-brainstorm` only handles synthesis/ideation.
- If the HTML template becomes too large or hard to maintain, keep `report.md` canonical and simplify `report.html` to a visual shell that mirrors only the main sections and tables.
