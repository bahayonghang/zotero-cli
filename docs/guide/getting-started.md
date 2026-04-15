# 快速开始

## 项目包含什么

这个仓库是一个 Rust workspace，核心目标是提供一个围绕 Zotero 的 CLI：

- 本地读取 `zotero.sqlite`
- 读取 PDF 与工作区内容
- 通过 Zotero Web API 执行写操作
- 做 workspace / RAG / preprint 状态同步

命令入口在 `src/zot-cli/src/main.rs`，文档中的命令面与它保持一致。

## 两种启动方式

优先选择一种调用路径，并在同一轮任务里保持一致：

```bash
zot --json doctor
```

如果系统里还没有安装 `zot`：

```bash
cargo run -q -p zot-cli -- --json doctor
```

## 什么时候先跑 doctor

以下情况都建议先执行 `doctor`：

- 第一次接触当前环境
- 任何写操作前
- PDF 提取有问题
- workspace 索引或 query 出现问题
- 用户反馈“为什么不工作”

推荐命令：

```bash
zot --json doctor
```

## 本地构建与安装

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
- 工作区目录：`~/.config/zot/workspaces`

常见环境变量：

- `ZOT_DATA_DIR`
- `ZOT_LIBRARY_ID`
- `ZOT_API_KEY`
- `ZOT_EMBEDDING_URL`
- `ZOT_EMBEDDING_KEY`
- `ZOT_EMBEDDING_MODEL`
- `SEMANTIC_SCHOLAR_API_KEY`
- `S2_API_KEY`

## 文档站本地预览

文档站本身基于 VitePress，位于 `docs/`：

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
- [Skills 总览](/skills/overview)
- [故障排查](/cli/troubleshooting)
