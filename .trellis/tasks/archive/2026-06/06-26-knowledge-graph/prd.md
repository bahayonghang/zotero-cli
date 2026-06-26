# 本地 Zotero 知识图谱与可视化分析

## Goal

在现有 `zot` CLI 上新增 `zot graph` 能力：为**全库或指定 collection** 构建一张**本地关系知识图谱**（论文为节点；边来自合著、共享标签、同 collection、Zotero 显式相关条目），计算基础结构分析指标，并通过**本地 web server + 交互式单页前端**进行可视化展示。全程只读本地 `zotero.sqlite`，离线可用，不依赖网络与 API key。

用户价值：把一个 Zotero 库/合集里「谁和谁相关、有哪些研究簇、哪些是枢纽文献/高产作者」一眼看清，辅助文献综述与选题。

## Background / Confirmed Facts（来自代码勘探）

- 4 crate workspace：`zot-core`（模型/配置/错误/envelope，轻依赖、无持久化）、`zot-local`（只读 `zotero.sqlite`）、`zot-remote`（网络）、`zot-cli`（clap 命令）。命令在 `commands/mod.rs` 分发，clap 面在 `cli.rs` / `cli/args.rs`。
- 本地读取已是**分块批量、无 N+1**：`LocalLibrary::search`（空 query 取全部 itemID）→ `get_items_batch`（db.rs:1578）→ `load_item_creators_batch`（db.rs:1723）。`zot_core::Item` 已自带 `creators` / `tags` / `collections`，三类边数据一次取齐。
- Zotero 显式相关条目存于 `itemRelations`（`predicateID = 1`）；`get_related_items`（db.rs:1154）已有「显式关联 + collection + tag」加权与 `HAVING cnt >= 2` 阈值先例可复用。
- 节点过滤先例：现有查询已排除 `attachment` / `note` / `annotation`（db.rs:181 等）。
- 输出：`--json` 走 `print_enveloped`（统一 envelope）；人类可读走 `format.rs`。
- `open_target()`（util.rs:53）已封装 `opener::open`，可直接开浏览器。
- **当前无任何 web server**（`mcp serve` 仅 scaffold、返回 `mcp-not-implemented`）；`tokio` 为 `full`。
- workspace lints：`unsafe_code = forbid`、`unwrap_used = deny`、`todo = deny`、`dbg_macro = deny` —— 新增非测试代码不得出现 `unwrap`/`expect`/`todo!`。

## Requirements

### R1 — 构图命令（数据）
`zot graph [--collection <KEY>] [--json]` 按 scope 构建知识图谱（缺省=当前 `--library` 全库），`--json` 时输出包含 `nodes` + `edges` + `metrics` 的统一 envelope。

### R2 — 节点
节点仅为**真实文献条目**（排除 attachment/note/annotation）。每个节点字段：`key`、`title`、`item_type`、`year`（可空）、`first_author`（可空）、`degree`、`weighted_degree`、`community`（社区 id）。

### R3 — 边
边带权且分类型 `relation ∈ {coauthor, tag, collection, related}`：
- `coauthor`：共享作者；
- `tag`：共享标签，阈值「共享标签数 ≥ 2」；
- `collection`：同 collection 共现；
- `related`：`itemRelations` 显式相关（最强信号）。

为避免 hairball：对成员数超过上限 `max_group_size`（默认 50）的「热门标签 / 大 collection」分组跳过或降权（详见 design.md）。同一对节点的多类型关系合并为一条边，保留 `weight`（加权和）与各 `relation` 明细。

### R4 — 分析指标
`metrics` 至少包含：`node_count`、`edge_count`、`connected_components`、度中心性 Top-N（`top_by_degree`）、加权度 Top-N、社区列表（标签传播算法，含每社区规模与代表标签）、`top_authors`、`top_tags`。`--json` 全量输出；人类可读输出一段摘要。

### R5 — 可视化服务（前台展示）
`zot graph serve [--collection <KEY>] [--port <N>] [--no-open]`：
- **启动时一次性构图**（同 R1–R4），不做服务端实时重查；
- 起一个仅绑定 `127.0.0.1` 的极小 HTTP 服务，提供静态 SPA 与一份 `graph.json`；
- 缺省自动打开浏览器（`--no-open` 关闭），前台运行至 `Ctrl-C` 干净退出，并在 stdout 打印访问 URL；
- 端口默认 `7878`，被占用时回退到系统分配的空闲端口。

### R6 — 前端交互（全部客户端完成，离线可用）
SPA 至少支持：图布局渲染、缩放/平移、节点搜索、按年份/标签/类型/最小度过滤、按社区着色、点击节点显示详情面板（标题/作者/年份/类型/标签/DOI/URL 链接）。可视化库**内联打包**，无需联网、无 CDN 依赖。

### R7 — 分层落位
- 图相关共享类型（`GraphNode` / `GraphEdge` / `EdgeRelation` / `GraphMetrics` / `KnowledgeGraph` / `GraphOptions`）放 `zot-core`（仅 serde，无新依赖）；
- 构图与图算法放 `zot-local` 新模块 `graph.rs`（`LocalLibrary` 方法 + 纯函数）；
- 命令、HTTP 服务与前端静态资源放 `zot-cli`。

### R8 — 测试
- 新命令面 CLI parse 测试（`graph`、`graph serve` 及其 flag）；
- 基于现有 SQLite fixture（db.rs 内置 Vaswani/BERT 等）的构图单测，断言节点/边/指标结构与数值自洽。

## Acceptance Criteria

- [ ] 在测试 fixture 上 `zot graph --json` 返回结构良好的 envelope：`nodes` 非空、每条 `edge` 有 `source`/`target`/`relation`/`weight`、`metrics` 字段齐全。
- [ ] 指标自洽：`connected_components ≤ node_count`；每个节点 `degree` 等于其关联边数；`edge_count` 等于去重后边数。
- [ ] `zot graph --collection <KEY> --json` 的节点集合是该 collection 条目的子集（不含库内其它条目）。
- [ ] `zot graph`（无 `--json`）打印人类可读摘要（节点/边/社区数 + Top 文献/作者/标签），不 panic。
- [ ] `zot graph serve --no-open --port <p>`：`GET /` 返回 200 HTML，`GET /graph.json` 返回 200 且为合法 JSON；进程前台运行，`Ctrl-C` 干净退出（退出码 0）。
- [ ] 前端在断网环境下仍能正常渲染与交互（资源内联，无外链）。
- [ ] `cargo clippy --all-targets` 在 workspace deny-lints 下通过（新增非测试代码无 `unwrap`/`expect`/`todo!`）。
- [ ] `cargo test` 通过，含 R8 的 parse 测试与构图单测。

## Out of Scope（MVP 不做，列为 Phase 2 候选）

- **引文图谱（who-cites-whom）**：需 `zot-remote` 接 Semantic Scholar references/citations，受 API key / 限流 / DOI 覆盖率制约。
- **服务端实时重查**：浏览器内不重启切换 scope/过滤、按需展开邻居（MVP 为启动时一次性构图）。
- **介数中心性 / Louvain 社区**：需引入 `petgraph`（MVP 用手写度/连通分量/标签传播）。
- **作者 / 标签作为一等节点**（多类型二部图）。
- **任何写回 Zotero** 的操作。

## Open Questions（非阻塞，已取默认值，评审可改）

- 是否拆为父任务 + 2 子任务（A 构图/分析、B serve/SPA）。默认：**单任务两片实现**（先 A 后 B）。
- 可视化库：默认 **Cytoscape.js**（内联离线）。
- HTTP 服务实现：默认 **`tiny_http`**（极小、同步，规避在 deny-unwrap 下手写 HTTP 的风险）。
- 指标深度：默认**基础指标、不引入 petgraph**。
