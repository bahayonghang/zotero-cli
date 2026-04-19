<!--
  GitHub Topics 建议（在 Settings → About → Topics 里手动添加）：
  zotero, zotero-cli, zotero-api, zotero-integration, rust, rust-cli, cli,
  pdf-extraction, rag, semantic-search, vector-search, bm25, hybrid-search,
  research-tools, reference-manager, citation, scholar, ai-agents, mcp,
  claude-code, llm, command-line, terminal, developer-tools
-->

<div align="center">

# zot

**面向研究者、终端爱好者与 AI Agent 的 Rust 原生 Zotero 命令行。**

用一个二进制文件搜索本地 Zotero 文献库、提取 PDF 正文与批注、维护语义检索 workspace、并通过 Zotero Web API 执行受控写操作。

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-orange.svg?logo=rust)](./Cargo.toml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-red.svg?logo=rust)](./Cargo.toml)
[![平台](https://img.shields.io/badge/platform-macOS_|_Linux_|_Windows-lightgrey.svg)](#安装)
[![Zotero](https://img.shields.io/badge/Zotero-7-CC2936.svg)](https://www.zotero.org)
[![文档](https://img.shields.io/badge/docs-VitePress-42b883.svg)](./docs/index.md)
[![Agent 原生](https://img.shields.io/badge/agent--native-JSON_envelope-8A2BE2.svg)](#原生适配-ai-agent)
[![欢迎 PR](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](#贡献)

[English](./README.md) · [简体中文](./README.zh-CN.md) · [文档（中文）](./docs/index.md) · [Docs (EN)](./docs/en/index.md)

</div>

---

## 为什么是 zot

Zotero 是最好的开源文献管理器，但它的命令行体验长期缺位。`zot` 补上这一块：

- **终端原生。** 一个 Rust 二进制，不依赖 Python 或 Node，装一次到处能脚本化。
- **Agent 原生。** 每条命令都能返回稳定的 JSON envelope（`{"ok": true, "data": ...}`），Claude Code、Cursor 或任何 MCP 风格的 Agent 都能直接消费。
- **库级直读。** 直接读 `zotero.sqlite` 和附件 `storage/`，不需要导出，也不依赖桌面端。
- **PDF 可用。** 通过 Pdfium 提取正文、批注、outline，避免研究流程卡在“重新 OCR 一次”。
- **检索齐全。** 库级和 workspace 级都支持 BM25、semantic、hybrid 三种检索模式。
- **写入受控。** 所有写操作都走 Zotero Web API，配合 `doctor` 前置检查和 dry-run 分级，`zotero.sqlite` 永远不会被直接改。
- **可嵌入 AI。** 自带 `zot-skills` skill，Claude Code agent 能把 Zotero 任务自动路由到正确命令。

如果你尝试过用 grep、RAG、LLM 处理自己的 Zotero 库但最后放弃了，`zot` 就是为这种人做的。

---

## 核心能力

| 领域 | 能做什么 |
| --- | --- |
| **本地搜索** | 按 query、精确 tag、creator、year、item type、collection、Better BibTeX citation key 搜索 |
| **库浏览** | 枚举 tags、libraries、groups、feeds、feed items |
| **PDF 提取** | 正文、分页 annotation、outline、children |
| **库级语义索引** | 建库级向量索引，`semantic-search` 支持 BM25 / semantic / hybrid |
| **阅读 workspace** | 主题级 BM25 + 向量索引（`workspace new / add / import / index / query / search`） |
| **条目新增** | DOI、URL、本地 PDF 三种路径，支持 `--attach-mode auto\|linked-url\|none` 和 OA PDF 解析 |
| **写操作** | notes、tags、collection 成员、重复条目合并、Scite retractions、Semantic Scholar 补全 |
| **诊断** | `doctor` 一次性报告 DB、PDF backend、Better BibTeX、embedding、写权限、feeds、annotation 能力 |
| **AI skill** | `skills/zot-skills/SKILL.md` 把自然语言 Zotero 请求路由到正确的 `zot` 命令 |

---

## 安装

### 用 GitHub 安装 CLI

```bash
cargo install --git https://github.com/bahayonghang/zotero-cli.git zot-cli --locked
```

### 用 `npx skills add` 安装 `zot-skills`

```bash
npx skills add https://github.com/bahayonghang/zotero-cli --skill zot-skills
```

这会从仓库里的 [`skills/zot-skills/SKILL.md`](./skills/zot-skills/SKILL.md) 安装内置 skill。

### 首次运行

```bash
zot --json doctor
```

`doctor` 一次告诉你：SQLite 是否可读、Pdfium 是否可用、写权限是否就绪、是否已有 semantic index、Better BibTeX / embedding / feeds 是否配置好。

### 未安装时

```bash
cargo run -q -p zot-cli -- --json doctor
```

---

## 30 秒上手

```bash
# 环境诊断
zot --json doctor

# 字段搜索
zot --json library search "attention" \
    --tag transformer --creator Vaswani --year 2017

# 按 Better BibTeX citation key 直接定位
zot --json library citekey Smith2024

# 拉取单条目的 PDF、outline、annotation、children
zot --json item get      ATTN001
zot --json item outline  ATTN001
zot --json item children ATTN001
zot --json item annotation list --item-key ATTN001

# 用 DOI 新增条目，有开放获取 PDF 时自动附上
zot --json item add-doi 10.1038/nature12373 --tag reading --attach-mode auto

# 库级语义检索
zot --json library semantic-index  --fulltext
zot --json library semantic-search "mechanistic interpretability" \
    --mode hybrid --limit 5

# 主题级 workspace + RAG 风格检索
zot --json workspace new    llm-safety
zot --json workspace import llm-safety --search "reward hacking"
zot --json workspace index  llm-safety
zot --json workspace query  llm-safety \
    "主要的失败模式有哪些？" --mode hybrid --limit 5
```

每条命令都支持 `--json`。envelope 固定：

```json
{ "ok": true, "data": { "...": "..." }, "meta": { "...": "..." } }
```

```json
{ "ok": false, "error": { "code": "...", "message": "...", "hint": "..." } }
```

---

## 原生适配 AI Agent

仓库自带 Claude Code skill：[`skills/zot-skills/SKILL.md`](./skills/zot-skills/SKILL.md)。用上面的 `npx skills add` 装好后，自然语言请求就会自动落到 `zot`：

- _“找 Vaswani 2017 年带 transformer 标签的论文”_ → `library search`
- _“把 ATTN001 的批注全部拿出来”_ → `item annotation list`
- _“给我整理一个 LLM 安全的 RAG 检索面”_ → `workspace new / import / index`
- _“这篇预印本已经正式发表了吗？”_ → `sync update-status`

skill 里同时写死了安全边界：`doctor` 前置、合并重复条目必须先 dry-run、写操作要显式授权、绝对不直接改 `zotero.sqlite`。

---

## 能力对比

| 能力 | `zot`（本项目） | Zotero 桌面端 | `pyzotero` 脚本 | `ref/zotero-mcp`（旧） |
| --- | --- | --- | --- | --- |
| 直接读本地 SQLite | 支持 | 不适用 | 不支持（只有 Web API） | 部分（Python） |
| 原生 PDF 正文 + annotation + outline | 支持（Pdfium） | 需要手动复制 | 自己写 | 部分 |
| BM25 + semantic + hybrid 检索 | 支持 | 不支持 | 自己写 | 部分 |
| 主题 workspace + 索引 | 支持 | 不支持 | 自己写 | 不支持 |
| DOI / URL 导入 + OA PDF attach-mode | 支持 | 部分 | 自己写 | 支持 |
| Scite retractions / Semantic Scholar 补全 | 支持 | 不支持 | 自己写 | 支持 |
| 稳定的 JSON envelope | 支持 | 不支持 | 自己写 | 部分 |
| 内置 Claude Code skill | 支持 | 不支持 | 不支持 | 不支持 |
| 单一静态二进制 | 支持（Rust） | GUI 程序 | 需要 Python 环境 | 需要 Python 环境 |

`zot` 是旧 `ref/zotero-mcp` 原型的 CLI-first 继任者。旧的 MCP connector 风格接口被故意映射成显式的 `zot` 命令。

---

## 工作区结构

Rust workspace 位于 `src/`：

| Crate | 职责 |
| --- | --- |
| [`src/zot-core`](./src/zot-core) | 共享配置、模型、错误、JSON envelope |
| [`src/zot-local`](./src/zot-local) | SQLite 读取、PDF helper、workspace 与本地索引逻辑 |
| [`src/zot-remote`](./src/zot-remote) | Zotero Web API、Better BibTeX、OA PDF 解析、Scite、embeddings |
| [`src/zot-cli`](./src/zot-cli) | `zot` 二进制与命令入口 |

仓库级 lint 禁用 `unsafe`、`dbg!`、`todo!`、`unwrap()`。

---

## 配置

配置文件：`~/.config/zot/config.toml`

### 环境变量

| 变量 | 用途 |
| --- | --- |
| `ZOT_DATA_DIR` | 覆盖 Zotero 数据目录 |
| `ZOT_LIBRARY_ID` | Web API 写操作使用的库 id |
| `ZOT_API_KEY` | Zotero Web API key（写操作必填） |
| `ZOT_EMBEDDING_URL` | 兼容 OpenAI 协议的 embedding 端点 |
| `ZOT_EMBEDDING_KEY` | embedding provider key |
| `ZOT_EMBEDDING_MODEL` | embedding 模型名 |
| `SEMANTIC_SCHOLAR_API_KEY` / `S2_API_KEY` | Semantic Scholar 访问 |

### 可选覆盖

`ZOT_BBT_PORT`、`ZOT_BBT_URL`、`ZOT_SCITE_API_BASE`、`ZOT_CROSSREF_API_BASE`、`ZOT_UNPAYWALL_API_BASE`、`ZOT_PMC_API_BASE`、`ZOT_SEMANTIC_SCHOLAR_GRAPH_BASE`。

---

## 文档

仓库自带双语 VitePress 文档站：

- 中文快速开始：[docs/guide/getting-started.md](./docs/guide/getting-started.md)
- 中文 CLI 总览：[docs/cli/overview.md](./docs/cli/overview.md)
- 中文 Skills 总览：[docs/skills/overview.md](./docs/skills/overview.md)
- English getting started: [docs/en/guide/getting-started.md](./docs/en/guide/getting-started.md)
- English CLI overview: [docs/en/cli/overview.md](./docs/en/cli/overview.md)

本地预览：

```bash
just docs    # npm install + vitepress dev
```

正式版通过 [`.github/workflows/deploy-docs.yml`](./.github/workflows/deploy-docs.yml) 在 release 时自动部署到 GitHub Pages。

---

## 当前边界

- `zot mcp serve` 仅是占位 scaffold，会返回 `mcp-not-implemented`，MCP 相关工作流暂时走 CLI。
- annotation 创建是 PDF-first，依赖本地 PDF、Pdfium、写权限。
- `library citekey` 优先用 Better BibTeX，不可用时会退回 Extra 字段解析。
- 旧 MCP 原型里的 connector 风格 `search` / `fetch` 不会作为独立命令，而是映射到 `library search`、`item get` 等现有命令。

---

## 验证

```bash
just ci
```

按顺序执行：`cargo fmt --all --check` → `cargo check --workspace` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`。

---

## 贡献

欢迎 issue、可复现的 bug 报告和 PR。先看仓库的协作契约：[`AGENTS.md`](./AGENTS.md)，再使用 [`.github/ISSUE_TEMPLATE`](./.github/ISSUE_TEMPLATE) 和 [`.github/PULL_REQUEST_TEMPLATE.md`](./.github/PULL_REQUEST_TEMPLATE.md) 里的模板。

提 PR 前：

1. 本地跑 `just ci`。
2. 命令面改动同步更新 `docs/` 和 `docs/en/`。
3. 保持 `skills/zot-skills/SKILL.md` 与 CLI 一致。

---

## 致谢

- [Zotero](https://www.zotero.org)：本项目依托的开源文献管理器。
- [Better BibTeX](https://retorque.re/zotero-better-bibtex/)：citation key 与 JSON-RPC。
- [Pdfium](https://pdfium.googlesource.com/pdfium/) / [`pdfium-render`](https://crates.io/crates/pdfium-render)：PDF 正文与 outline 提取。
- [Semantic Scholar](https://www.semanticscholar.org)、[Scite](https://scite.ai)、[Unpaywall](https://unpaywall.org)、[Crossref](https://www.crossref.org)、[OA PMC](https://www.ncbi.nlm.nih.gov/pmc/)：远程补全与开放获取资源解析。

---

## 许可协议

[MIT](./LICENSE) —— 研究工作应该能自由迁移。
