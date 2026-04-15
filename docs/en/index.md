---
layout: home

hero:
  name: zot
  text: Rust Zotero CLI Docs
  tagline: Bilingual guides for the CLI and zot-skills
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
  - title: Source-aligned
    details: The command surface is documented from src/zot-cli/src/main.rs, not from stale prose.
  - title: Full CLI coverage
    details: Covers doctor, library, item, collection, workspace, sync, and current mcp status.
  - title: Skill workflows
    details: Covers triggers, routing, safety boundaries, typical flows, and fallbacks for zot-skills.
---

## Start here

- New to the project: read [Getting Started](/en/guide/getting-started)
- Want to run commands: read [CLI Overview](/en/cli/overview)
- Want to guide an AI/agent workflow: read [Skills Overview](/en/skills/overview)

## Scope

This site focuses on two things:

1. How to use the Rust `zot` CLI
2. How to apply the operational rules in `skills/zot-skills/SKILL.md`

If docs and implementation disagree, prefer these sources:

- `src/zot-cli/src/main.rs`
- `README.md`
- `skills/zot-skills/SKILL.md`
- `AGENTS.md`
