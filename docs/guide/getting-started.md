# 快速开始

## 先记住这个定位

这个仓库的主交互面是 `zot-skills`，不是命令列表。

- `zot-skills` 负责理解用户想从 Zotero 里拿什么内容
- Rust `zot` 负责真正执行读取、检索、索引和写入
- CLI 页面是参考面，主要给排障、脚本化和直接调用使用

如果你是让 Agent 去“找文献、读 PDF、提取批注、整理主题 workspace、更新条目”，就按 skill-first 的方式启动。

## 这个 skill 能从 Zotero 里拿到什么

- 条目元数据：title、creator、year、item type、citation、children
- 证据内容：PDF fulltext、outline、annotations、notes
- 组织结构：tags、collections、libraries、feeds
- 主题工作面：workspace、semantic index、semantic query/search
- 受控写入：notes、tags、collections、imports、duplicate merge、publication status sync

## 推荐启动顺序

### 1. 安装 skill

```bash
npx skills add https://github.com/bahayonghang/zotero-cli --skill zot-skills
```

### 2. 提供运行时

```bash
cargo install --git https://github.com/bahayonghang/zotero-cli.git zot-cli --locked
```

### 3. 先跑一次 doctor

```bash
zot --json doctor
```

如果你正在这个仓库里开发，而 `zot` 还没有安装到 `PATH`：

```bash
cargo run -q -p zot-cli -- --json doctor
```

同一轮任务固定一种调用路径，不要混用。

### 4. 需要远端写入或保存查询时，先配 config

如果你后面要做这些事：

- 写 note / tag / collection
- 保存或删除 saved search
- 做 publication status sync

可以先初始化配置：

```bash
zot config init --library-id <你的 library id> --api-key <你的 api key>
```

如果你想给一个独立 profile 配置：

```bash
zot config init --target-profile work --library-id <你的 library id> --api-key <你的 api key> --make-default
```

## 可以直接怎么提需求

- “找出我库里 reward hacking 相关的论文”
- “把这篇论文的 PDF 批注和 note 拿出来”
- “给我建一个 llm-safety workspace，再把相关论文导进去”
- “检查这篇预印本是否已经正式发表”
- “给这篇文献加一条 note，再打上 priority 标签”

这些都是 `zot-skills` 应该直接接管的请求。

更完整的开口方式，见：[Agent 用法](/skills/agent-usage)

## 什么时候退回直接命令

以下场景更适合直接跑运行时：

- 你在排环境问题
- 你在验证某个子命令的实际返回
- 你在写脚本或回归测试
- 你需要确认 `doctor`、索引、写权限等前置状态

常见起点：

```bash
zot --json doctor
zot --json library search "reward hacking" --limit 10
zot --json item get ATTN001
zot --json item annotation list --item-key ATTN001
zot --json workspace query llm-safety "主要的失败模式有哪些？" --mode hybrid --limit 5
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

重点关注这些字段：

- `db_exists`
- `write_credentials.configured`
- `pdf_backend.available`
- `better_bibtex.available`
- `libraries.feeds_available`
- `semantic_index`
- `annotation_support`
- `embedding.configured`

## 仓库开发命令

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

- [Agent 用法](/skills/agent-usage)
- [Skills 总览](/skills/overview)
- [典型工作流](/skills/workflows)
- [路由策略](/skills/routing)
- [CLI 总览](/cli/overview)
