# Skills Overview

Here, “skills” mainly means:

- `skills/zot/SKILL.md`: the runtime contract for Zotero lookup, extraction, organization, and safe writes
- `skills/zot-brainstorm/SKILL.md`: reference-grounded gap analysis, brainstorming, innovation directions, and local reports from real Zotero sources

They are not a second CLI tutorial. They are Zotero workflow contracts for Claude Code, Codex, and similar agents.

If your goal is to:

- find items in Zotero
- pull PDF text, annotations, notes, or child items
- build a long-lived workspace
- save a search
- download attachments
- update the library safely

start from the skill, not the command list.

The skill keeps local SQLite / Local HTTP reads, desktop-bridge merge/dedupe, and explicit Web API mutations as separate routes. Desktop failures do not fall back automatically, and unsupported local tag, note, or collection writes are never invented.

If your goal is to:

- brainstorm from real Zotero collections, workspaces, or explicit item keys
- analyze research defects, evidence limits, and next-step innovation points
- generate local `report.md` and `report.html` outputs by default

use `zot-brainstorm`.

## Read this before the CLI pages

Recommended order:

1. [Agent Usage](/en/skills/agent-usage)
2. [Routing](/en/skills/routing)
3. [Safety](/en/skills/safety)
4. [Workflows](/en/skills/workflows)
5. If you used one of the reference CLIs before, read [Migrating from ref\zotero-cli](/en/guide/migrating-from-ref-zotero-cli) or [Migrating from ref\zotagent](/en/guide/migrating-from-ref-zotagent)
6. Only then, if needed, [CLI Overview](/en/cli/overview)

## What the skill treats as first-class

- Item metadata: title, creators, year, item type, citation, child items
- Evidence: PDF full text, outline, annotations, notes
- Organization: tags, collections, libraries, feeds, saved searches
- Working sets: workspaces, semantic indexing, semantic query/search
- Configuration and troubleshooting: doctor, config, profiles
- Controlled writes: notes, tags, collections, imports, duplicate merge, publication-status sync
- Literature synthesis: reference-grounded brainstorming, defect analysis, innovation ranking, local Markdown/HTML reports

## How the agent should think about it

The skill decides four things first:

1. what Zotero content the user actually wants
2. whether the task is read-only or mutating
3. whether `doctor` should run first
4. whether the reply should return results, evidence, boundaries, or a failure reason

From the user side, the right move is not:

- “Which command should I run?”

It is:

- “Find papers in my Zotero library about …”
- “Pull the annotations and notes for this item”
- “Create a workspace and make it ready for Q&A”
- “Show me the current config and default profile first”

## When this skill should not trigger

By default, do not use it for:

- generic literature search
- ordinary paper summarization
- bibliography-format teaching
- PDF work that does not depend on Zotero or a local workspace

Those requests do not treat Zotero as the primary content source.

## Related files

- Main skill file: `skills/zot/SKILL.md`
- Regression prompts: `skills/zot/test-prompts.json`
- Eval set: `skills/zot/evals/evals.json`
- Brainstorm skill file: `skills/zot-brainstorm/SKILL.md`
- Brainstorm report templates: `skills/zot-brainstorm/templates/report.md` and `skills/zot-brainstorm/templates/report.html`
