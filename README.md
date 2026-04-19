<!--
  GitHub topics to add via repo settings (Settings → About → Topics):
  zotero, zotero-cli, zotero-api, zotero-integration, rust, rust-cli, cli,
  pdf-extraction, rag, semantic-search, vector-search, bm25, hybrid-search,
  research-tools, reference-manager, citation, scholar, ai-agents, mcp,
  claude-code, llm, command-line, terminal, developer-tools
-->

<div align="center">

# zot

**The Rust-first Zotero CLI for researchers, terminal-lovers, and AI agents.**

Search your local Zotero library, extract PDF text and annotations, build semantic reading workspaces, and drive authenticated Zotero Web API writes — from one fast, scriptable binary.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-orange.svg?logo=rust)](./Cargo.toml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-red.svg?logo=rust)](./Cargo.toml)
[![Platform](https://img.shields.io/badge/platform-macOS_|_Linux_|_Windows-lightgrey.svg)](#install)
[![Zotero](https://img.shields.io/badge/Zotero-7-CC2936.svg)](https://www.zotero.org)
[![Docs](https://img.shields.io/badge/docs-VitePress-42b883.svg)](./docs/en/index.md)
[![Agent-native](https://img.shields.io/badge/agent--native-JSON_envelope-8A2BE2.svg)](#ai-agent-native)
[![PRs welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](#contributing)

[English](./README.md) · [简体中文](./README.zh-CN.md) · [Docs (EN)](./docs/en/index.md) · [文档（中文）](./docs/index.md)

</div>

---

## Why zot?

Zotero is the best open reference manager. It is not a great command line. `zot` fills that gap:

- **Terminal-native.** A single Rust binary. No Python runtime, no Node, no browser. Install once, script everywhere.
- **Agent-native.** Every command can emit a strict JSON envelope (`{"ok": true, "data": ...}`), so Claude Code, Cursor, and MCP-style agents can consume output reliably.
- **Library-aware.** Reads `zotero.sqlite` and the attachment `storage/` directory directly — no export, no round-trip through the desktop app.
- **PDF-aware.** Pulls full text, annotations, and outlines via Pdfium. No "please re-OCR" dead ends for research workflows.
- **Retrieval-aware.** First-class BM25, semantic, and hybrid search on both the whole library and per-topic reading workspaces.
- **Write-safe.** Mutations go through the Zotero Web API behind `doctor` gates and dry-run flags — `zotero.sqlite` is never touched.
- **Drop-in for AI.** Ships the `zot-skills` skill so Claude Code agents route Zotero tasks to the right command without prompting.

If you have ever tried to grep, RAG, or LLM-summarize your Zotero library and given up, `zot` is built for you.

---

## Features

| Area | What you get |
| --- | --- |
| **Local search** | Query by text, exact tag, creator, year, item type, collection, or Better BibTeX citation key |
| **Library browse** | Enumerate tags, libraries, groups, feeds, feed items |
| **PDF extraction** | Full-text, page-aware annotations, outline, child items |
| **Semantic library index** | Build vector indexes over the whole library; run `semantic-search` with BM25 / semantic / hybrid modes |
| **Reading workspaces** | Per-topic BM25 + vector indexes (`workspace new / add / import / index / query / search`) |
| **Item creation** | Import by DOI, URL, or local PDF with `--attach-mode auto\|linked-url\|none` and OA PDF resolution |
| **Write ops** | Notes, tags, collection membership, duplicate merge, Scite retractions, Semantic Scholar enrichment |
| **Diagnostics** | `doctor` reports DB, PDF backend, Better BibTeX, embeddings, write credentials, feeds, annotation support |
| **AI skill bundle** | `skills/zot-skills/SKILL.md` routes natural-language Zotero requests to the right `zot` command |

---

## Install

### From source (recommended today)

```bash
git clone <this-repo>
cd zotero-cli
just build     # cargo build --release -p zot-cli
just install   # cargo install --path src/zot-cli --locked --force
```

### First run

```bash
zot --json doctor
```

`doctor` tells you, in one envelope, whether the SQLite database is reachable, whether Pdfium is available, whether write credentials are configured, whether a semantic index exists, and whether Better BibTeX / embeddings / feeds are ready.

### Without installing

```bash
cargo run -q -p zot-cli -- --json doctor
```

---

## 30-second tour

```bash
# Environment check
zot --json doctor

# Library search with field filters
zot --json library search "attention" \
    --tag transformer --creator Vaswani --year 2017

# Jump straight to a paper by Better BibTeX citation key
zot --json library citekey Smith2024

# Pull PDF text, outline, annotations, and children for one item
zot --json item get      ATTN001
zot --json item outline  ATTN001
zot --json item children ATTN001
zot --json item annotation list --item-key ATTN001

# Import a paper by DOI and auto-attach an open-access PDF when available
zot --json item add-doi 10.1038/nature12373 --tag reading --attach-mode auto

# Library-level semantic search
zot --json library semantic-index  --fulltext
zot --json library semantic-search "mechanistic interpretability" \
    --mode hybrid --limit 5

# Build a per-topic reading workspace with RAG-style retrieval
zot --json workspace new    llm-safety
zot --json workspace import llm-safety --search "reward hacking"
zot --json workspace index  llm-safety
zot --json workspace query  llm-safety \
    "What are the main failure modes?" --mode hybrid --limit 5
```

Every command honors `--json`. The envelope is stable:

```json
{ "ok": true, "data": { "...": "..." }, "meta": { "...": "..." } }
```

```json
{ "ok": false, "error": { "code": "...", "message": "...", "hint": "..." } }
```

---

## AI-agent-native

`zot` ships a Claude Code skill at [`skills/zot-skills/SKILL.md`](./skills/zot-skills/SKILL.md). Drop it into any agent runtime and natural-language requests route to `zot` automatically:

- _"find papers tagged `transformer` by Vaswani in 2017"_ → `library search`
- _"pull the annotations out of ATTN001"_ → `item annotation list`
- _"give me a RAG-ready index for everything I've read on LLM safety"_ → `workspace new / import / index`
- _"is this preprint officially published yet?"_ → `sync update-status`

Hard safety rules are part of the skill: `doctor` gates, dry-run for duplicate merges, explicit permission for writes, and no direct edits to `zotero.sqlite`.

---

## How it compares

| Capability | `zot` (this project) | Zotero desktop UI | `pyzotero` scripts | `ref/zotero-mcp` (legacy) |
| --- | --- | --- | --- | --- |
| Direct local SQLite reads | Yes | n/a | No (API-only) | Partial (Python) |
| Native PDF text + annotations + outline | Yes (Pdfium) | Manual copy | DIY | Partial |
| BM25 + semantic + hybrid search | Yes | No | DIY | Partial |
| Per-topic reading workspace with index | Yes | No | DIY | No |
| DOI / URL import with OA PDF attach-mode | Yes | Partial | DIY | Yes |
| Scite retractions / Semantic Scholar enrichment | Yes | No | DIY | Yes |
| Stable JSON envelope for agents | Yes | No | DIY | Partial |
| Bundled Claude Code skill | Yes | No | No | No |
| Single static binary | Yes (Rust) | GUI app | Python env | Python env |

`zot` is a CLI-first successor to the legacy `ref/zotero-mcp` prototype. The old MCP connector-style tools are intentionally reshaped into explicit `zot` commands.

---

## Workspace layout

Rust workspace lives under `src/`:

| Crate | Role |
| --- | --- |
| [`src/zot-core`](./src/zot-core) | Shared config, models, errors, JSON envelope |
| [`src/zot-local`](./src/zot-local) | SQLite reads, PDF helpers, workspace and local index logic |
| [`src/zot-remote`](./src/zot-remote) | Zotero Web API, Better BibTeX, OA PDF resolution, Scite, embeddings |
| [`src/zot-cli`](./src/zot-cli) | The `zot` binary and command surface |

Repo-wide lints forbid `unsafe`, `dbg!`, `todo!`, and `unwrap()`.

---

## Configuration

Config file: `~/.config/zot/config.toml`

### Environment variables

| Variable | Purpose |
| --- | --- |
| `ZOT_DATA_DIR` | Override Zotero data directory |
| `ZOT_LIBRARY_ID` | Numeric library id for Web API writes |
| `ZOT_API_KEY` | Zotero Web API key (required for writes) |
| `ZOT_EMBEDDING_URL` | OpenAI-compatible embedding endpoint |
| `ZOT_EMBEDDING_KEY` | Embedding provider key |
| `ZOT_EMBEDDING_MODEL` | Embedding model name |
| `SEMANTIC_SCHOLAR_API_KEY` / `S2_API_KEY` | Semantic Scholar access |

### Optional overrides

`ZOT_BBT_PORT`, `ZOT_BBT_URL`, `ZOT_SCITE_API_BASE`, `ZOT_CROSSREF_API_BASE`, `ZOT_UNPAYWALL_API_BASE`, `ZOT_PMC_API_BASE`, `ZOT_SEMANTIC_SCHOLAR_GRAPH_BASE`.

---

## Docs

A full bilingual VitePress site ships with the repo:

- Get started (EN): [docs/en/guide/getting-started.md](./docs/en/guide/getting-started.md)
- CLI overview (EN): [docs/en/cli/overview.md](./docs/en/cli/overview.md)
- Skills & routing (EN): [docs/en/skills/overview.md](./docs/en/skills/overview.md)
- 中文快速开始：[docs/guide/getting-started.md](./docs/guide/getting-started.md)
- 中文 CLI 总览：[docs/cli/overview.md](./docs/cli/overview.md)

Run locally:

```bash
just docs    # npm install + vitepress dev
```

Released versions are published to GitHub Pages via [`.github/workflows/deploy-docs.yml`](./.github/workflows/deploy-docs.yml).

---

## Current boundary

- `zot mcp serve` is a scaffold and currently returns `mcp-not-implemented`. MCP-style workflows go through the CLI for now.
- Annotation creation is PDF-first: it needs a local PDF, Pdfium, and write credentials.
- `library citekey` uses Better BibTeX when available and falls back to Extra-field parsing.
- Connector-style `search` / `fetch` tools from the legacy MCP prototype are reshaped into `library search`, `item get`, etc. — there is no separate connector surface.

---

## Verification

```bash
just ci
```

Runs `cargo fmt --all --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` in order.

---

## Contributing

Issues, reproducible bug reports, and PRs are welcome. Please read [`AGENTS.md`](./AGENTS.md) for the repo's operating contract, then use the templates in [`.github/ISSUE_TEMPLATE`](./.github/ISSUE_TEMPLATE) and [`.github/PULL_REQUEST_TEMPLATE.md`](./.github/PULL_REQUEST_TEMPLATE.md).

Before opening a PR:

1. Run `just ci` locally.
2. Update docs under `docs/` (and `docs/en/`) if you change a command surface.
3. Keep the skill in `skills/zot-skills/SKILL.md` consistent with the CLI.

---

## Acknowledgments

- [Zotero](https://www.zotero.org) — the open reference manager this tool stands on.
- [Better BibTeX](https://retorque.re/zotero-better-bibtex/) — citation keys and JSON-RPC.
- [Pdfium](https://pdfium.googlesource.com/pdfium/) via [`pdfium-render`](https://crates.io/crates/pdfium-render) — PDF text and outline extraction.
- [Semantic Scholar](https://www.semanticscholar.org), [Scite](https://scite.ai), [Unpaywall](https://unpaywall.org), [Crossref](https://www.crossref.org), [OA PMC](https://www.ncbi.nlm.nih.gov/pmc/) — remote enrichment and OA resolution.

---

## License

[MIT](./LICENSE) — research should be portable.
