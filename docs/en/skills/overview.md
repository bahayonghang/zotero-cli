# Skills Overview

Here, “skills” means `skills/zot-skills/SKILL.md`.

It is not a second CLI tutorial. It is the Zotero workflow contract for Claude Code, Codex, and similar agents.

If your goal is to:

- find items in Zotero
- pull PDF text, annotations, notes, or child items
- build a long-lived workspace
- save a search
- download attachments
- update the library safely

start from the skill, not the command list.

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

- Main skill file: `skills/zot-skills/SKILL.md`
- Regression prompts: `skills/zot-skills/test-prompts.json`
- Eval set: `skills/zot-skills/evals/evals.json`
