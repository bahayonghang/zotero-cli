<!--
  GitHub Topics 建议（在 Settings → About → Topics 里手动添加）：
  zotero, zotero-cli, zotero-api, zotero-integration, rust, rust-cli, cli,
  pdf-extraction, rag, semantic-search, vector-search, bm25, hybrid-search,
  research-tools, reference-manager, citation, scholar, ai-agents, mcp,
  claude-code, llm, command-line, terminal, developer-tools
-->

<div align="center">

# zot

**面向 Agent 的 Zotero skill 运行时，用来查询、阅读并安全更新库里的内容。**

把已有的 Zotero 文献库变成稳定的 AI 工作面：找条目、读 PDF 证据、提取批注和笔记、建立主题 workspace、并在安全门下执行 Zotero Web API 写操作。

<img src="./docs/public/images/zot-icon.png" alt="zot 图标" width="180" />

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-orange.svg?logo=rust)](./Cargo.toml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-red.svg?logo=rust)](./Cargo.toml)
[![平台](https://img.shields.io/badge/platform-macOS_|_Linux_|_Windows-lightgrey.svg)](#推荐的-agent-启动方式)
[![Zotero](https://img.shields.io/badge/Zotero-7-CC2936.svg)](https://www.zotero.org)
[![文档](https://img.shields.io/badge/docs-VitePress-42b883.svg)](./docs/index.md)
[![Agent 原生](https://img.shields.io/badge/agent--native-JSON_envelope-8A2BE2.svg)](#直接用自然语言提出-zotero-任务)
[![欢迎 PR](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](#贡献)

[English](./README.md) · [简体中文](./README.zh-CN.md) · [文档（中文）](./docs/index.md) · [Docs (EN)](./docs/en/index.md)

</div>

---

## 这个仓库真正想解决什么

`zot` 分两层：

- `skills/zot-skills/SKILL.md` 是主交互面。你想让 Claude Code 或类似 Agent 用自然语言处理 Zotero 任务时，先装它。
- Rust `zot` 二进制是 skill 背后的执行层。人也可以直接调用它做排障、脚本化或本地验证。

如果你的真实目标是“把 Zotero 里已经有的论文、笔记、标签、PDF、批注、feeds 用起来”，就先从 skill 出发，不要先背子命令。

这个定位和 Zotero 自己的能力边界一致：

- Zotero 的库数据在 `zotero.sqlite` 和 `storage/` 附件目录里。
- 条目本身携带 metadata、notes、tags、attachments 等结构化内容。
- Zotero 批注可以进入 note，并带回源 PDF 页面的链接。
- 写操作走 Web API，需要带写权限的凭证和版本控制。

`zot` 做的事，就是把这套模型变成 Agent 可稳定调用的工作流：本地直读内容，远端受控写入，边界明确。

---

## 这个 skill 能拿 Zotero 里的什么内容

| 你真正想做什么 | `zot-skills` 能提供什么 |
| --- | --- |
| 找到对的文献 | 按 query、tag、creator、year、collection、citation key、library、feed 找条目 |
| 读取证据面 | 返回 item metadata、children、citation、PDF 正文、outline、notes、annotations |
| 建一个主题工作面 | 创建 workspace、导入匹配论文、建索引、再做 query/search |
| 复用整个文献库 | 建库级 semantic index，跑 BM25 / semantic / hybrid 检索 |
| 安全改库 | 写 notes、tags、collection 关系、导入条目、合并重复项、同步发表状态 |
| 让 Agent 不乱写 | 先跑 `doctor`、强制 dry-run、安全门、稳定 JSON envelope、绝不直接改 `zotero.sqlite` |

---

## 推荐的 Agent 启动方式

先装 skill，再提供它调用的运行时。

### 1. 安装 skill

```bash
npx skills add https://github.com/bahayonghang/zotero-cli --skill zot-skills
```

这会安装仓库里内置的工作流契约：[`skills/zot-skills/SKILL.md`](./skills/zot-skills/SKILL.md)。

### 2. 安装运行时

```bash
cargo install --git https://github.com/bahayonghang/zotero-cli.git zot-cli --locked
```

### 3. 跑一次环境检查

```bash
zot --json doctor
```

如果你就在这个仓库里开发，而 `zot` 还没进 `PATH`，用：

```bash
cargo run -q -p zot-cli -- --json doctor
```

同一轮任务里固定一种调用方式，不要来回切。

### 4. 需要写入或 saved search 时，先初始化 config

如果你后面要写 note、tag、collection 关系、saved search 或 publication status：

```bash
zot config init --library-id <你的 library id> --api-key <你的 api key>
```

如果你想单独建一个 profile：

```bash
zot config init --target-profile work --library-id <你的 library id> --api-key <你的 api key> --make-default
```

---

## 直接用自然语言提出 Zotero 任务

装好 skill 后，首选交互面是用户请求，不是命令列表。

- “找 2017 年 Vaswani 写的、带 `transformer` 标签的论文。”
- “把 `ATTN001` 的 PDF 批注和子笔记都拉出来。”
- “给我建一个 `llm-safety` workspace，把 reward hacking 相关论文都导进去。”
- “查一下这篇预印本现在有没有正式发表版本。”
- “给这篇文献加一条 note，再打上 `priority` 标签。”  
  这类写操作要在用户明确授权后再做。

skill 会把这些请求路由到 `library`、`item`、`collection`、`workspace` 或 `sync`，并决定是否先跑 `doctor`。

更完整的自然语言开口方式，见：

- Agent 用法（中文）：[docs/skills/agent-usage.md](./docs/skills/agent-usage.md)
- Agent Usage (EN): [docs/en/skills/agent-usage.md](./docs/en/skills/agent-usage.md)

---

## 直接看运行时参考

如果你要手动排障，或要直接驱动运行时，这几条通常是起点：

```bash
zot --json doctor
zot --json library search "reward hacking" --limit 10
zot --json item get ATTN001
zot --json item annotation list --item-key ATTN001
zot --json workspace query llm-safety "主要的失败模式有哪些？" --mode hybrid --limit 5
```

运行时的顶层 envelope 固定不变：

```json
{ "ok": true, "data": { "...": "..." }, "meta": { "...": "..." } }
```

```json
{ "ok": false, "error": { "code": "...", "message": "...", "hint": "..." } }
```

---

## 文档怎么读

双语文档站现在按 skill-first 的 Zotero 工作流来组织，CLI 页面只保留为参考面：

- Skills 总览（中文）：[docs/skills/overview.md](./docs/skills/overview.md)
- Agent 用法（中文）：[docs/skills/agent-usage.md](./docs/skills/agent-usage.md)
- 典型工作流（中文）：[docs/skills/workflows.md](./docs/skills/workflows.md)
- 快速开始（中文）：[docs/guide/getting-started.md](./docs/guide/getting-started.md)
- CLI 参考（中文）：[docs/cli/overview.md](./docs/cli/overview.md)
- Skills overview (EN): [docs/en/skills/overview.md](./docs/en/skills/overview.md)
- Agent Usage (EN): [docs/en/skills/agent-usage.md](./docs/en/skills/agent-usage.md)
- Skill workflows (EN): [docs/en/skills/workflows.md](./docs/en/skills/workflows.md)
- Getting started (EN): [docs/en/guide/getting-started.md](./docs/en/guide/getting-started.md)
- CLI reference (EN): [docs/en/cli/overview.md](./docs/en/cli/overview.md)

本地预览：

```bash
just docs
```

正式文档通过 [`.github/workflows/deploy-docs.yml`](./.github/workflows/deploy-docs.yml) 发布到 GitHub Pages。

---

## 当前边界

- `zot mcp serve` 现在只是 scaffold，会返回 `mcp-not-implemented`。当前应走 skill + runtime。
- 本地读取来自 Zotero 数据目录。写操作只走 Zotero Web API。
- annotation 创建是 PDF-first，依赖本地 PDF、Pdfium 和写凭证。
- citation key 查询优先走 Better BibTeX，可用时补强；否则退回兼容的本地解析。
- 旧参考实现里的 `search` / `fetch` 这种 connector 心智模型，已经被显式映射到 `library`、`item`、`collection`、`workspace`、`sync` 这些工作流。

---

## 验证

```bash
just ci
```

会执行 `cargo fmt --all --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`。

---

## 贡献

欢迎提 issue、可复现 bug 和 PR。先看 [`AGENTS.md`](./AGENTS.md) 里的协作约束，再使用 [`.github/ISSUE_TEMPLATE`](./.github/ISSUE_TEMPLATE) 和 [`.github/PULL_REQUEST_TEMPLATE.md`](./.github/PULL_REQUEST_TEMPLATE.md)。

提 PR 前：

1. 本地跑 `just ci`。
2. 如果改了用户可见工作流，同步更新 `docs/` 和 `docs/en/`。
3. 保持 [`skills/zot-skills/SKILL.md`](./skills/zot-skills/SKILL.md) 和运行时行为一致。

---

## 致谢

- [Zotero](https://www.zotero.org)：本项目依托的开源文献管理器与数据模型。
- [Better BibTeX](https://retorque.re/zotero-better-bibtex/)：citation key 工作流。
- [Pdfium](https://pdfium.googlesource.com/pdfium/) / [`pdfium-render`](https://crates.io/crates/pdfium-render)：PDF 正文和 outline 提取。
- [Semantic Scholar](https://www.semanticscholar.org)、[Scite](https://scite.ai)、[Unpaywall](https://unpaywall.org)、[Crossref](https://www.crossref.org)、[OA PMC](https://www.ncbi.nlm.nih.gov/pmc/)：补全和开放获取解析。

---

## 许可协议

[MIT](./LICENSE) —— 文献工作流应该能自由迁移。
