<!--
  GitHub topics to add via repo settings (Settings → About → Topics):
  zotero, zotero-cli, zotero-api, zotero-integration, rust, rust-cli, cli,
  pdf-extraction, rag, semantic-search, vector-search, bm25, hybrid-search,
  research-tools, reference-manager, citation, scholar, ai-agents, mcp,
  claude-code, llm, command-line, terminal, developer-tools
-->

<div align="center">

# zot

**An agent-first Zotero skill runtime for querying, reading, and safely updating library content.**

Turn an existing Zotero library into a dependable content surface for AI workflows: find items, read PDF evidence, extract annotations and notes, build topic workspaces, merge duplicates through Zotero Desktop, and perform gated Zotero Web API writes.

<img src="./docs/public/images/zot-icon.png" alt="zot icon" width="180" />

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-orange.svg?logo=rust)](./Cargo.toml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-red.svg?logo=rust)](./Cargo.toml)
[![Platform](https://img.shields.io/badge/platform-macOS_|_Linux_|_Windows-lightgrey.svg)](#recommended-agent-setup)
[![Zotero](https://img.shields.io/badge/Zotero-7-CC2936.svg)](https://www.zotero.org)
[![Docs](https://img.shields.io/badge/docs-VitePress-42b883.svg)](./docs/en/index.md)
[![Agent-native](https://img.shields.io/badge/agent--native-JSON_envelope-8A2BE2.svg)](#ask-for-zotero-work-in-plain-language)
[![PRs welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](#contributing)

[English](./README.md) · [简体中文](./README.zh-CN.md) · [Docs (EN)](./docs/en/index.md) · [文档（中文）](./docs/index.md)

</div>

---

## What this repo is really for

`zot` has two layers:

- `skills/zot/SKILL.md` is the main operator surface. Install it when you want Claude Code or a similar agent to handle Zotero work from plain-language requests.
- The Rust `zot` binary is the execution layer behind that skill. Humans can run it directly for debugging, scripting, or local verification.

If the real goal is “use the papers, notes, tags, PDFs, annotations, or feeds already inside Zotero”, start from the skill. The CLI is the runtime, not the main mental model.

This matches the underlying products:

- Zotero stores library data in `zotero.sqlite` plus attachment files under `storage/`.
- Zotero items carry metadata, notes, tags, attachments, and other related data.
- Zotero annotations can be turned into notes with links back to the source PDF page.
- Zotero Desktop's built-in connector can import BibTeX or RIS into the selected writable target; other supported mutations use the Web API with write-scoped credentials and version checks.

`zot` mirrors that structure for agents: read local content directly, import new records through the built-in connector, and route merge/dedupe plus other mutations through the Web API.

---

## What the skill can do with Zotero content

| What you want              | What the `zot` skill can surface                                                                              |
| -------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Find the right source      | Search by query, tag, creator, year, collection, citation key, library, or feed                               |
| Read the evidence          | Return item metadata, child items, citations, PDF text, outline, notes, and annotations                       |
| Build a working set        | Create a topic workspace, import matching papers, index it, and query it                                      |
| Reuse full-library context | Build a library semantic index and run BM25 / semantic / hybrid retrieval                                     |
| Update the library safely  | Add notes, tags, collection membership, imports, duplicate merges, and status sync with explicit permission   |
| Keep the agent honest      | Run `doctor`, enforce dry-run gates, emit a stable JSON envelope, and never write to `zotero.sqlite` directly |

---

## Recommended agent setup

Install the skill first. Then provide the runtime it calls.

### 1. Install the skill

```bash
npx skills add https://github.com/bahayonghang/zotero-cli --skill zot
```

This installs the bundled workflow contract from [`skills/zot/SKILL.md`](./skills/zot/SKILL.md).

### 2. Install the runtime

```bash
cargo install --git https://github.com/bahayonghang/zotero-cli.git zot-cli --locked
```

### 3. Run one environment check

```bash
zot --json doctor
```

If you are working inside this repository and `zot` is not on `PATH`, use:

```bash
cargo run -q -p zot-cli -- --json doctor
```

Keep one invocation path for the whole session. Do not switch back and forth.

`doctor` reports `local_sqlite_read`, `local_http_read`, `connector_write`, and `web_write` independently. Web credentials are optional for local reads and connector imports, but required for merge/dedupe and other Web API mutations.

If local PDF reads need Pdfium and no compatible library is already present, `zot` will auto-download a managed Pdfium binary on the first local PDF read on supported Windows, macOS, and glibc Linux targets.

### 4. Import through Zotero Desktop's built-in connector

```bash
zot --json item import --file references.bib
zot --json item import --file references.bib --confirm
```

No plugin or pairing is required. Zotero must be running, and the library or collection selected in its UI must be writable. Without `--confirm`, the command validates and previews the import without sending records.

### 5. Initialize Web config for merge/dedupe and other writes

If you plan to write notes, tags, collection membership, saved searches, or publication status:

```bash
zot config init --library-id <your library id> --api-key <your api key>
```

For a named profile:

```bash
zot config init --target-profile work --library-id <your library id> --api-key <your api key> --make-default
```

---

## Ask for Zotero work in plain language

Once the skill is installed, the preferred interface is the user request, not the subcommand list.

- “Find papers tagged `transformer` by Vaswani from 2017.”
- “Pull the PDF annotations and child notes for `ATTN001`.”
- “Build me a workspace for LLM safety and import papers about reward hacking.”
- “Check whether this preprint has an official publication record now.”
- “Add a note and a `priority` tag to this item.”  
  Only after explicit permission for writes.

The skill routes these requests to the right `library`, `item`, `collection`, `workspace`, or `sync` workflow and decides when `doctor` is required first.

See the agent phrasing guide here:

- Agent Usage (EN): [docs/en/skills/agent-usage.md](./docs/en/skills/agent-usage.md)
- Agent 用法（中文）：[docs/skills/agent-usage.md](./docs/skills/agent-usage.md)

---

## Direct runtime reference

If you need to debug or drive the runtime manually, these are the usual starting points:

```bash
zot --json doctor
zot --json library search "reward hacking" --limit 10
zot --json library recent --count 10
zot --json library dedupe --collection COLL001
zot --json item get ATTN001
zot --json item merge KEEP001 DUPE001
zot --json item annotation list --item-key ATTN001
zot --json workspace query llm-safety "What are the main failure modes?" --mode hybrid --limit 5
zot completions powershell
```

The runtime always returns the same top-level envelope:

```json
{ "ok": true, "data": { "...": "..." }, "meta": { "...": "..." } }
```

```json
{ "ok": false, "error": { "code": "...", "message": "...", "hint": "..." } }
```

---

## Docs

The bilingual docs are organized around skill-first Zotero workflows, with CLI pages kept as reference:

- Skills overview (EN): [docs/en/skills/overview.md](./docs/en/skills/overview.md)
- Agent usage (EN): [docs/en/skills/agent-usage.md](./docs/en/skills/agent-usage.md)
- Skill workflows (EN): [docs/en/skills/workflows.md](./docs/en/skills/workflows.md)
- Getting started (EN): [docs/en/guide/getting-started.md](./docs/en/guide/getting-started.md)
- Migrating from ref\zotero-cli (EN): [docs/en/guide/migrating-from-ref-zotero-cli.md](./docs/en/guide/migrating-from-ref-zotero-cli.md)
- CLI reference (EN): [docs/en/cli/overview.md](./docs/en/cli/overview.md)
- Skills 总览（中文）：[docs/skills/overview.md](./docs/skills/overview.md)
- Agent 用法（中文）：[docs/skills/agent-usage.md](./docs/skills/agent-usage.md)
- 典型工作流（中文）：[docs/skills/workflows.md](./docs/skills/workflows.md)
- 快速开始（中文）：[docs/guide/getting-started.md](./docs/guide/getting-started.md)
- 从 ref\zotero-cli 迁移（中文）：[docs/guide/migrating-from-ref-zotero-cli.md](./docs/guide/migrating-from-ref-zotero-cli.md)
- CLI 参考（中文）：[docs/cli/overview.md](./docs/cli/overview.md)

Local preview:

```bash
just docs
```

Released docs are published to GitHub Pages via [`.github/workflows/deploy-docs.yml`](./.github/workflows/deploy-docs.yml).

---

## Current boundary

- `zot mcp serve` is scaffolded and currently returns `mcp-not-implemented`. For now, use the skill plus the runtime.
- Local SQLite and Zotero Local HTTP are read-only. Never use either as a write transport.
- The built-in connector only imports new BibTeX/RIS records into Zotero's currently selected writable target; it does not update or merge existing items.
- Merge/dedupe, note, tag, collection, Web import, annotation, saved-search, and status-sync mutations use the Zotero Web API.
- Legacy `desktop_bridge` and `write_backend` config entries are ignored; `doctor` reports a migration hint so they can be removed safely.
- Merge/dedupe is preview-first. Batch dedupe skips low-confidence groups unless the user separately authorizes `--include-low-confidence` after reviewing them.
- `item tag batch` is preview-first and requires `--confirm`; confirmed targets cannot exceed
  `--max-affected` (default 50), and partial per-operation failures are returned explicitly.
- Annotation creation is PDF-first. It requires a local PDF, Pdfium support, and write credentials.
- Citation-key lookup prefers Better BibTeX support and falls back to compatible local parsing when possible.
- Legacy connector-style `search` / `fetch` ideas are intentionally mapped onto explicit `library`, `item`, `collection`, `workspace`, and `sync` workflows.

If you need to override Pdfium resolution manually:

- `ZOT_PDFIUM_LIB_PATH` or `PDFIUM_LIB_PATH` can point to a compatible Pdfium library file or directory.
- `ZOT_PDFIUM_CACHE_DIR` can override the base directory used for Zot-managed Pdfium downloads.

---

## Verification

```bash
just ci
```

This runs `cargo fmt --all --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the canonical skill mirror check.

---

## Contributing

Issues, reproducible bug reports, and PRs are welcome. Read [`AGENTS.md`](./AGENTS.md) first for the repo contract, then use the templates in [`.github/ISSUE_TEMPLATE`](./.github/ISSUE_TEMPLATE) and [`.github/PULL_REQUEST_TEMPLATE.md`](./.github/PULL_REQUEST_TEMPLATE.md).

Before opening a PR:

1. Run `just ci` locally.
2. Update `docs/` and `docs/en/` if you change a user-facing workflow.
3. Keep [`skills/zot/SKILL.md`](./skills/zot/SKILL.md) aligned with the runtime behavior.

---

## Acknowledgments

- [Zotero](https://www.zotero.org) — the open reference manager and data model this project builds on.
- [Better BibTeX](https://retorque.re/zotero-better-bibtex/) — citation-key workflows.
- [Pdfium](https://pdfium.googlesource.com/pdfium/) via [`pdfium-render`](https://crates.io/crates/pdfium-render) — PDF text and outline extraction.
- [Semantic Scholar](https://www.semanticscholar.org), [Scite](https://scite.ai), [Unpaywall](https://unpaywall.org), [Crossref](https://www.crossref.org), [OA PMC](https://www.ncbi.nlm.nih.gov/pmc/) — enrichment and open-access resolution.

---

## License

[MIT](./LICENSE) — research workflows should stay portable.
