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

把已有的 Zotero 文献库变成稳定的 AI 工作面：找条目、读 PDF 证据、提取批注和笔记、建立主题 workspace、通过 Zotero Desktop 合并重复项，并在安全门下执行 Zotero Web API 写操作。

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

- `skills/zot/SKILL.md` 是主交互面。你想让 Claude Code 或类似 Agent 用自然语言处理 Zotero 任务时，先装它。
- Rust `zot` 二进制是 skill 背后的执行层。人也可以直接调用它做排障、脚本化或本地验证。

如果你的真实目标是“把 Zotero 里已经有的论文、笔记、标签、PDF、批注、feeds 用起来”，就先从 skill 出发，不要先背子命令。

这个定位和 Zotero 自己的能力边界一致：

- Zotero 的库数据在 `zotero.sqlite` 和 `storage/` 附件目录里。
- 条目本身携带 metadata、notes、tags、attachments 等结构化内容。
- Zotero 批注可以进入 note，并带回源 PDF 页面的链接。
- 配对后的 desktop bridge 可调用 Zotero 原生合并；其他已支持 mutation 走带写权限和版本控制的 Web API。

`zot` 把这套模型变成 Agent 可稳定调用的工作流：本地直读内容，merge/dedupe 按选定的 desktop 或 Web backend 执行，其他 mutation 保持 Web 路径，backend 失败后绝不自动切换。

---

## 这个 skill 能拿 Zotero 里的什么内容

| 你真正想做什么 | `zot` skill 能提供什么 |
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
npx skills add https://github.com/bahayonghang/zotero-cli --skill zot
```

这会安装仓库里内置的工作流契约：[`skills/zot/SKILL.md`](./skills/zot/SKILL.md)。

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

`doctor` 会分别报告 `local_sqlite_read`、`local_http_read`、`desktop_write`、`web_write`，并给出有效的 `selected_write_backend`。本地读取和已配对的 desktop merge/dedupe 不要求 Web 凭据。

如果本地 PDF 读取需要 Pdfium，而当前机器上还没有可用库，`zot` 会在受支持的 Windows、macOS、glibc Linux 平台上，在第一次本地 PDF 读取时自动下载受管 Pdfium。

### 4. 本机 merge/dedupe 先配对 Zotero Desktop

```bash
zot --json bridge setup
```

`bridge setup` 只生成内置 XPI 并打开所在目录。用户需要在 Zotero 里手动安装、重启，然后使用 Zotero UI 显示的五分钟单次配对码：

```bash
zot --json bridge pair PAIR-CODE
zot --json bridge status
```

desktop backend 当前只支持 `item merge`、`library duplicates-merge` 和 `library dedupe`，不是任意本机写通道。

### 5. 其他写入或 saved search 使用 Web config

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
zot --json bridge status
zot --json library search "reward hacking" --limit 10
zot --json library recent --count 10
zot --json library dedupe --collection COLL001
zot --json item get ATTN001
zot --json item merge KEEP001 DUPE001
zot --json item annotation list --item-key ATTN001
zot --json workspace query llm-safety "主要的失败模式有哪些？" --mode hybrid --limit 5
zot completions powershell
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
- 从 ref\zotero-cli 迁移（中文）：[docs/guide/migrating-from-ref-zotero-cli.md](./docs/guide/migrating-from-ref-zotero-cli.md)
- CLI 参考（中文）：[docs/cli/overview.md](./docs/cli/overview.md)
- Skills overview (EN): [docs/en/skills/overview.md](./docs/en/skills/overview.md)
- Agent Usage (EN): [docs/en/skills/agent-usage.md](./docs/en/skills/agent-usage.md)
- Skill workflows (EN): [docs/en/skills/workflows.md](./docs/en/skills/workflows.md)
- Getting started (EN): [docs/en/guide/getting-started.md](./docs/en/guide/getting-started.md)
- Migrating from ref\zotero-cli (EN): [docs/en/guide/migrating-from-ref-zotero-cli.md](./docs/en/guide/migrating-from-ref-zotero-cli.md)
- CLI reference (EN): [docs/en/cli/overview.md](./docs/en/cli/overview.md)

本地预览：

```bash
just docs
```

正式文档通过 [`.github/workflows/deploy-docs.yml`](./.github/workflows/deploy-docs.yml) 发布到 GitHub Pages。

---

## 当前边界

- `zot mcp serve` 现在只是 scaffold，会返回 `mcp-not-implemented`。当前应走 skill + runtime。
- 本地 SQLite 和 Zotero Local HTTP 都只读，不能作为写通道。
- desktop 写入当前只覆盖 merge/dedupe；note、tag、collection、import、annotation、saved-search、status-sync mutation 仍走 Web API。
- `--write-backend desktop|web` 为当前调用选择一个后端；失败保留在原后端，不自动 fallback。
- 从未配对 bridge 的旧 profile 继续默认使用 Web。
- merge/dedupe 默认先 preview；批量 dedupe 默认跳过 low-confidence，只有单独展示并取得明确风险授权后才可使用 `--include-low-confidence`。
- annotation 创建是 PDF-first，依赖本地 PDF、Pdfium 和写凭证。
- citation key 查询优先走 Better BibTeX，可用时补强；否则退回兼容的本地解析。
- 旧参考实现里的 `search` / `fetch` 这种 connector 心智模型，已经被显式映射到 `library`、`item`、`collection`、`workspace`、`sync` 这些工作流。

如果你要手工覆盖 Pdfium 查找：

- `ZOT_PDFIUM_LIB_PATH` 或 `PDFIUM_LIB_PATH` 可以指向兼容的 Pdfium 库文件或目录。
- `ZOT_PDFIUM_CACHE_DIR` 可以覆盖 Zot 受管 Pdfium 下载使用的基础缓存目录。

---

## 验证

```bash
just ci
```

会执行 `cargo fmt --all --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 和 canonical skill 镜像检查。

---

## 贡献

欢迎提 issue、可复现 bug 和 PR。先看 [`AGENTS.md`](./AGENTS.md) 里的协作约束，再使用 [`.github/ISSUE_TEMPLATE`](./.github/ISSUE_TEMPLATE) 和 [`.github/PULL_REQUEST_TEMPLATE.md`](./.github/PULL_REQUEST_TEMPLATE.md)。

提 PR 前：

1. 本地跑 `just ci`。
2. 如果改了用户可见工作流，同步更新 `docs/` 和 `docs/en/`。
3. 保持 [`skills/zot/SKILL.md`](./skills/zot/SKILL.md) 和运行时行为一致。

---

## 致谢

- [Zotero](https://www.zotero.org)：本项目依托的开源文献管理器与数据模型。
- [Better BibTeX](https://retorque.re/zotero-better-bibtex/)：citation key 工作流。
- [Pdfium](https://pdfium.googlesource.com/pdfium/) / [`pdfium-render`](https://crates.io/crates/pdfium-render)：PDF 正文和 outline 提取。
- [Semantic Scholar](https://www.semanticscholar.org)、[Scite](https://scite.ai)、[Unpaywall](https://unpaywall.org)、[Crossref](https://www.crossref.org)、[OA PMC](https://www.ncbi.nlm.nih.gov/pmc/)：补全和开放获取解析。

---

## 许可协议

[MIT](./LICENSE) —— 文献工作流应该能自由迁移。
