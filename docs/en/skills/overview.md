# Skills Overview

Here, “skills” refers to `skills/zot-skills/SKILL.md`. It is not another CLI. It is an operational playbook for AI systems, agents, and human operators.

## What this skill solves

It helps the operator decide:

- which command family should handle the task: `library`, `item`, `collection`, `workspace`, or `sync`
- whether the task is read-only or will mutate the Zotero library
- when to run `doctor` first
- when to use advanced flows such as citation-key lookup, semantic search, feeds, annotations, or Scite

## Trigger scope

This skill should trigger only when the user explicitly wants to work through `zot`, a Zotero library, or a local workspace, for example:

- searching inside an existing Zotero library, exporting citations, reading PDFs, or inspecting annotations
- looking up an item by citation key
- building a semantic index or running semantic search
- listing libraries, feeds, or feed items
- managing notes, tags, collections, or duplicate merge flows
- creating annotations, checking Scite data, or syncing preprint status

It is **not** the default skill for generic literature search or paper summarization.

## How to use it

1. Treat it as the operational playbook for Zotero work
2. route by user intent to the right command family
3. run `doctor` first for new environments, writes, PDF tasks, semantic flows, Better BibTeX lookup, and failure reports

## Related files

- Main skill file: `skills/zot-skills/SKILL.md`
- Regression prompts: `skills/zot-skills/test-prompts.json`

Continue with:

- [Routing](/en/skills/routing)
- [Safety](/en/skills/safety)
- [Workflows](/en/skills/workflows)
- [Fallbacks](/en/skills/fallback)
