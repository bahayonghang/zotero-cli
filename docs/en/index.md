---
layout: home

hero:
  name: zot
  text: "Agent-First Zotero Docs"
  tagline: "Bilingual guides for using zot-skills to query, read, organize, and safely update Zotero content"
  image:
    src: /images/zot-icon.png
    alt: zot icon
    width: 220
  actions:
    - theme: brand
      text: "Skills Quickstart"
      link: /en/skills/overview
    - theme: alt
      text: "Workflows"
      link: /en/skills/workflows
    - theme: alt
      text: "CLI Reference"
      link: /en/cli/overview

features:
  - title: "Start from Zotero content"
    details: "The docs begin with items, PDFs, annotations, notes, collections, feeds, and workspaces, then fall through to command reference."
  - title: "Skill first, runtime second"
    details: "`skills/zot-skills/SKILL.md` is the workflow contract for agents. The Rust `zot` CLI is the execution layer behind it."
  - title: "Explicit write boundaries"
    details: "Doctor gates, Web API write credentials, safety checks, and the current `mcp serve` limitation are documented directly."
---

## Start here

- Want to know how to ask in Claude Code or Codex: read [Agent Usage](/en/skills/agent-usage)
- Want to know what the skill can surface from Zotero: read [Skills Overview](/en/skills/overview)
- Want one end-to-end flow: read [Workflows](/en/skills/workflows)
- If you used one of the reference CLIs before: read [Migrating from ref\zotero-cli](/en/guide/migrating-from-ref-zotero-cli) and [Migrating from ref\zotagent](/en/guide/migrating-from-ref-zotagent)
- Want the manual command reference: read [CLI Overview](/en/cli/overview)

## Scope

This site focuses on three things, in that order:

1. How agents use `zot-skills` to work with Zotero metadata, notes, PDFs, annotations, collections, feeds, and workspaces
2. How users should ask for Zotero work in Claude Code, Codex, and similar agent environments
3. The runtime prerequisites, safety boundaries, and response contracts of the Rust `zot` layer
4. Where to look when you need direct CLI debugging or manual invocation

If the docs ever disagree with the implementation, prefer:

- `skills/zot-skills/SKILL.md`
- `README.md`
- `README.zh-CN.md`
- `src/zot-cli/src/main.rs`
- `AGENTS.md`
