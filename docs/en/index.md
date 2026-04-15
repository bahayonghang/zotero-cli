---
layout: home

hero:
  name: zot
  text: Rust Zotero CLI Docs
  tagline: Bilingual guides for the CLI, workspaces, and zot-skills
  actions:
    - theme: brand
      text: Get Started
      link: /en/guide/getting-started
    - theme: alt
      text: CLI Usage
      link: /en/cli/overview
    - theme: alt
      text: Skills Usage
      link: /en/skills/overview

features:
  - title: Source aligned
    details: The docs track `src/zot-cli/src/main.rs`, `skills/zot-skills/SKILL.md`, and the root READMEs.
  - title: Covers the expanded surface
    details: Includes citation-key lookup, feeds, semantic search, annotations, Scite, duplicate merge, and attach-mode workflows.
  - title: Explicit boundaries
    details: The docs call out doctor preconditions, write-safety rules, and the current `mcp serve` limitation directly.
---

## Start here

- New to the project: read [Getting Started](/en/guide/getting-started)
- Ready to run commands: read [CLI Overview](/en/cli/overview)
- Guiding an AI or operator workflow: read [Skills Overview](/en/skills/overview)

## Scope

This site focuses on two things:

1. The Rust `zot` CLI command surface, prerequisites, and limits
2. The routing, safety, and fallback rules in `skills/zot-skills/SKILL.md`

If the docs ever disagree with the implementation, prefer:

- `src/zot-cli/src/main.rs`
- `README.md`
- `README.zh-CN.md`
- `skills/zot-skills/SKILL.md`
- `AGENTS.md`
