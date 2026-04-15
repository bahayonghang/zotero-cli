# Skills Overview

Here, “skills” refers to `skills/zot-skills/SKILL.md`. It is not another CLI. It is an operational guide for AI systems, agents, and human operators.

## What this skill solves

It helps the operator decide quickly:

- whether the task is a one-off lookup or a persistent workspace flow
- whether the task is local read-only or requires Zotero Web API writes
- when to run `doctor` first
- when a safety confirmation is required

## Trigger scope

According to `SKILL.md`, this skill is appropriate for requests involving:

- Zotero / paper libraries / references / citations / bibliography
- PDF attachments / collections / tags / notes
- reading workspaces / paper RAG
- requests like “find papers”, “export citations”, “organize Zotero”, “build a reading workspace”, “read PDF content”, or “sync preprint status”

## How to use it

1. Treat it as the operational playbook for Zotero work
2. Route by user intent to `library`, `item`, `collection`, `workspace`, or `sync`
3. Run `doctor` first for new environments, writes, PDF issues, or workspace problems

## Related files

- Main skill file: `skills/zot-skills/SKILL.md`
- Regression prompts: `skills/zot-skills/test-prompts.json`

Continue with:

- [Routing](/en/skills/routing)
- [Safety](/en/skills/safety)
- [Workflows](/en/skills/workflows)
