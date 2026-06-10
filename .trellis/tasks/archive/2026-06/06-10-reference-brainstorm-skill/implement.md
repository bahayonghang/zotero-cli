# Implementation Plan: Reference-grounded brainstorming skill

## Assumptions

- The new skill folder will be `skills/zot-brainstorm`, based on the existing TODO and the user's requested capability.
- MVP is additive and does not change Rust crates.
- MVP is read-only with respect to Zotero. It may write local Markdown/HTML report files.
- Saved reports generate both Markdown and HTML by default.
- Zotero collections, workspaces, and explicit item keys are all first-class MVP inputs.
- Generated report content follows the user's prompt language by default.

## Checklist

1. Create `skills/zot-brainstorm/`.
   - Verify: `rg --files skills/zot-brainstorm`

2. Write `skills/zot-brainstorm/SKILL.md`.
   - Include frontmatter name `zot-brainstorm`.
   - Describe trigger conditions: Zotero collection/workspace/item-key based literature brainstorming, gap analysis, innovation-point generation.
   - Define non-trigger cases: generic literature search, ordinary paper summary, non-Zotero PDF review.
   - Reuse the existing `zot` runtime contract without duplicating every command.
   - Include the required evidence workflow and output contract from `design.md`.
   - Define local report saving rules and make Zotero/vault writes explicit opt-in only.
   - State that saved reports default to both `report.md` and `report.html`.
   - State that generated report prose and visible section labels follow the user's prompt language unless the user asks for another language.
   - Verify: read the file and check no unsupported command names were introduced.

3. Add `skills/zot-brainstorm/templates/report.md`.
   - Include canonical Markdown sections from `design.md`.
   - Include placeholders for collections, item counts, evidence levels, traceability, and output metadata.
   - Verify: read the file and check it can be filled without inventing evidence.

4. Add `skills/zot-brainstorm/templates/report.html`.
   - Keep it self-contained and offline.
   - Use Evidence Dossier structure: evidence rail, source table, evidence vs inference, defect matrix, ranked innovation cards, traceability table.
   - Include accessible structure, table captions, focus styles, print styles, and no external resources.
   - Verify: search the file for `http://`, `https://`, `<script src`, `<link rel="stylesheet"`, `@import`, `fetch(`, `XMLHttpRequest`, and remote-font references.
   - If the local HTML artifact validator is available, run it against the template after filling placeholder sample content.

5. Add `skills/zot-brainstorm/test-prompts.json`.
   - Cover multi-collection input.
   - Cover ambiguous collection names.
   - Cover metadata-only evidence limits.
   - Cover workspace-backed brainstorming.
   - Cover explicit item-key brainstorming.
   - Cover mixed collection + workspace + item-key source sets.
   - Cover large broad review.
   - Cover local Markdown report saving.
   - Cover default HTML report generation alongside Markdown.
   - Cover Chinese prompt -> Chinese report behavior.
   - Cover English prompt -> English report behavior.
   - Cover refusal to write Zotero notes or vault pages without explicit permission.
   - Verify: JSON parses.

6. Add `skills/zot-brainstorm/evals/evals.json`.
   - Mirror the existing `skills/zot/evals/evals.json` structure.
   - Assert grounded references, evidence grading, traceability, local output path reporting, offline HTML constraints, and no fabricated commands.
   - Verify: JSON parses.

7. Update docs only if scope is approved for discoverability.
   - Candidate Chinese docs: `docs/skills/overview.md`, `docs/skills/workflows.md`, `docs/skills/examples.md`.
   - Candidate English docs: add or mirror an English examples page if desired, because `docs/en/skills/examples.md` does not currently exist.
   - Verify: docs links point to existing files.

8. Run formatting and lightweight validation.
   - `git diff --check`
   - JSON parse checks for new JSON files
   - HTML offline dependency grep for `templates/report.html`
   - If Rust/doc metadata changed unexpectedly, run `just ci`; otherwise document why it was not necessary for skill-only Markdown/JSON changes.

## Risky Files / Rollback Points

- `skills/zot-brainstorm/SKILL.md`: trigger description can overlap with `skills/zot`. Keep it narrower: synthesis/brainstorming only.
- `skills/zot-brainstorm/templates/report.html`: easy to overbuild. Keep it a portable offline template, not a dynamic app.
- Docs index files: avoid adding links to non-existent English pages.
- JSON eval files: keep valid JSON with no comments or trailing commas.

Rollback:

- Remove `skills/zot-brainstorm/`.
- Revert any docs links added for discoverability.

## Pre-start Review Gate

Before running `task.py start`, confirm:

- MVP persistence target is agreed.
- Planning artifacts have been reviewed or the user has explicitly approved implementation.
