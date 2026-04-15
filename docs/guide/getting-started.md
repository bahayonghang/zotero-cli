# 快速开始

## 项目是什么

这个仓库是一个 Rust workspace，用来提供一个 CLI-first 的 Zotero 工具链：

- 本地读取 `zotero.sqlite` 和附件 storage
- 提取 PDF 文本、outline、批注
- 通过 Zotero Web API 执行写操作
- 做 library-level semantic index/search 和 workspace 检索
- 执行 Better BibTeX citation key lookup、Scite 检查、preprint 状态同步

命令入口以 `src/zot-cli/src/main.rs` 为准。

## 两种启动方式

同一轮任务里只选一种调用路径：

```bash
zot --json doctor
```

如果还没有安装 `zot`：

```bash
cargo run -q -p zot-cli -- --json doctor
```

## 什么时候先跑 doctor

以下场景默认先执行 `doctor`：

- 第一次接触当前环境
- 任何写操作前
- PDF / outline / annotation 相关任务
- library semantic index/search
- workspace index/query 异常
- Better BibTeX citekey 查询
- 用户反馈“为什么不工作”

推荐命令：

```bash
zot --json doctor
```

重点关注这些字段：

- `write_credentials.configured`
- `pdf_backend.available`
- `better_bibtex.available`
- `libraries.feeds_available`
- `semantic_index`
- `annotation_support`
- `embedding.configured`

## 构建与安装

项目根目录常用命令：

```bash
just build
just install
just ci
```

`just ci` 会按顺序运行：

1. `cargo fmt --all --check`
2. `cargo check --workspace`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace`

## 配置位置

- 配置文件：`~/.config/zot/config.toml`
- workspace 根目录：`~/.config/zot/workspaces`

常用环境变量：

- `ZOT_DATA_DIR`
- `ZOT_LIBRARY_ID`
- `ZOT_API_KEY`
- `ZOT_EMBEDDING_URL`
- `ZOT_EMBEDDING_KEY`
- `ZOT_EMBEDDING_MODEL`
- `SEMANTIC_SCHOLAR_API_KEY`
- `S2_API_KEY`

可选集成覆盖：

- `ZOT_BBT_PORT`
- `ZOT_BBT_URL`
- `ZOT_SCITE_API_BASE`
- `ZOT_CROSSREF_API_BASE`
- `ZOT_UNPAYWALL_API_BASE`
- `ZOT_PMC_API_BASE`
- `ZOT_SEMANTIC_SCHOLAR_GRAPH_BASE`

## 文档站本地预览

文档站基于 VitePress，位于 `docs/`：

```bash
cd docs
npm install
npm run dev
```

生产构建：

```bash
cd docs
npm run build
```

## 下一步阅读

- [CLI 总览](/cli/overview)
- [library 命令](/cli/library)
- [item 命令](/cli/item)
- [Skills 总览](/skills/overview)
