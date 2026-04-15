# zot

[English](./README.md) | [简体中文](./README.zh-CN.md) | [文档（中文）](./docs/index.md) | [Docs (EN)](./docs/en/index.md)

`zot` 是一个 Rust 优先的 Zotero CLI，用来读取本地文献库、提取 PDF 内容、管理 reading workspace，并通过 Zotero Web API 执行写操作。它的目标是以 CLI-first 的方式覆盖旧的 `ref/zotero-mcp` 主要能力面，同时继续围绕 `library`、`item`、`collection`、`workspace`、`sync` 这几组命令组织工作流。

## 核心能力

- 从 `zotero.sqlite` 和附件 storage 读取本地 Zotero 数据
- 按 query、精确 tag、creator、year、type、collection、citation key 搜索条目
- 枚举 tags、libraries、feeds、feed items
- 提取 PDF 文本、批注、outline，并批量读取 item children
- 建立库级 semantic index，执行 semantic / hybrid search
- 通过 DOI、URL、文件新增条目，并用 `attach_mode` 控制 OA PDF 附件策略
- 管理 notes、tags、collections、重复条目合并，以及 Scite 检查
- 维护本地 reading workspace，支持 BM25、semantic、hybrid 检索

## 工作区结构

Rust workspace 位于 `src/`：

- `src/zot-core`：共享配置、模型、错误和 JSON envelope
- `src/zot-local`：SQLite 读取、PDF helper、workspace 和本地索引逻辑
- `src/zot-remote`：Zotero Web API、Better BibTeX、OA PDF 解析、Scite、embeddings
- `src/zot-cli`：`zot` 二进制和命令入口

## 快速开始

构建并安装：

```bash
just build
just install
```

第一次先做环境诊断：

```bash
zot --json doctor
```

如果系统里还没有安装 `zot`：

```bash
cargo run -q -p zot-cli -- --json doctor
```

## 常用命令

```bash
zot --json doctor
zot --json library search "attention" --tag transformer --creator Vaswani --year 2017
zot --json library citekey Smith2024
zot --json library semantic-status
zot --json library semantic-index --fulltext
zot --json library semantic-search "mechanistic interpretability" --mode hybrid --limit 5
zot --json item get ATTN001
zot --json item children ATTN001
zot --json item outline ATTN001
zot --json item add-doi 10.1038/nature12373 --tag reading --attach-mode auto
zot --json item annotation list --item-key ATTN001
zot --json item scite report --item-key ATTN001
zot --json collection search Transform
zot --json workspace query llm-safety "What are the main failure modes?" --mode hybrid --limit 5
```

## 配置

配置文件：

- `~/.config/zot/config.toml`

常用环境变量：

- `ZOT_DATA_DIR`
- `ZOT_LIBRARY_ID`
- `ZOT_API_KEY`
- `ZOT_EMBEDDING_URL`
- `ZOT_EMBEDDING_KEY`
- `ZOT_EMBEDDING_MODEL`
- `SEMANTIC_SCHOLAR_API_KEY`
- `S2_API_KEY`

可选的集成/测试覆盖变量：

- `ZOT_BBT_PORT`
- `ZOT_BBT_URL`
- `ZOT_SCITE_API_BASE`
- `ZOT_CROSSREF_API_BASE`
- `ZOT_UNPAYWALL_API_BASE`
- `ZOT_PMC_API_BASE`
- `ZOT_SEMANTIC_SCHOLAR_GRAPH_BASE`

## 文档入口

- 快速开始：[docs/guide/getting-started.md](./docs/guide/getting-started.md)
- CLI 总览：[docs/cli/overview.md](./docs/cli/overview.md)
- Skills 路由与安全：[docs/skills/overview.md](./docs/skills/overview.md)
- 英文文档首页：[docs/en/index.md](./docs/en/index.md)

## 当前边界

- `zot mcp serve` 仍然只是 scaffold，占位返回 unsupported
- 旧 MCP 里的 connector 风格 `search` / `fetch` 不会作为独立命令实现，而是映射到 CLI 工作流
- annotation 创建目前是 PDF-first，依赖本地 PDF 可读和写权限可用

## 验证

仓库统一校验命令：

```bash
just ci
```
