# Implement — 本地 Zotero 知识图谱与可视化分析

分两片交付，先 A 后 B，各自可独立验收/回滚。每步带验证命令。

## Slice A：构图 + 分析 + `--json`/人类输出（R1–R4, R7, R8 数据部分）

### A1 — zot-core 图类型

- 在 `src/zot-core/src/model.rs` 新增 `EdgeRelation`、`GraphNode`、`GraphEdge`、`CommunitySummary`、`RankedNode`、`GraphMetrics`、`KnowledgeGraph`、`GraphOptions`、`GraphRelationToggles`（serde derive，`rename_all = "kebab-case"` 与现有风格一致；复用 `TagSummary`）。
- 在 `src/zot-core/src/lib.rs` 重导出。
- 验证：`cargo build -p zot-core`。

### A2 — zot-local 构图与算法

- 新建 `src/zot-local/src/graph.rs`：
  - `impl LocalLibrary { pub fn build_knowledge_graph(&self, opts: &GraphOptions) -> ZotResult<KnowledgeGraph> }`；
  - 取节点复用 `self.search(SearchOptions{ collection, limit: usize::MAX, ..default })`；
  - 倒排建边 + `max_group_size` 跳过 + 同对合并；
  - 新增 `itemRelations`（predicateID=1）批量查询（`prepare_cached` + `sql_err`，遵守 zot-local DB 规范）；
  - 纯函数：`union_find_components`、`label_propagation`（确定性：按 key 排序遍历、平票取最小 id、固定轮数）、度/加权度、Top 榜与社区代表标签。
- 在 `src/zot-local/src/lib.rs` 导出 `build_knowledge_graph` 所需公共项。
- 验证：`cargo build -p zot-local`。

### A3 — zot-local 单测（R8 构图）

- 在 `src/zot-local/tests/` 新增 `graph_build.rs`（或并入现有 `search_regression.rs` 同款 fixture 装载方式），断言：节点排除 attachment/note/annotation；fixture 中合著/共享标签产生预期边；`degree == 关联边数`；`connected_components ≤ node_count`；label propagation 确定性（同输入两次结果一致）。
- 验证：`cargo test -p zot-local graph`。

### A4 — zot-cli `graph` 命令（无 serve）

- `src/zot-cli/src/cli.rs`：`Commands` 增 `Graph { command: GraphCommand }` 或直接 `Graph(GraphArgs)`（含可选子命令 `serve`，serve 在 Slice B 接实现）。
- `src/zot-cli/src/cli/args.rs`：`GraphArgs { collection: Option<String>, /* serve flags 占位 */ }`，遵循「长参数放 args.rs」约定。
- `src/zot-cli/src/commands/graph.rs`：`handle(ctx, args)` → `ctx.local_library()?.build_knowledge_graph(&opts)?`；`--json` 走 `print_enveloped`，否则 `format::print_graph_summary`。
- `src/zot-cli/src/commands/mod.rs`：分发接线。
- `src/zot-cli/src/format.rs`：新增 `print_graph_summary(&GraphMetrics)`。
- `src/zot-cli/src/cli.rs` 测试：在现有 parse 测试数组加 `["zot","graph"]`、`["zot","graph","--collection","COLTR02"]`。
- 验证：`cargo build`；`cargo test -p zot-cli`；`cargo run -- graph --json | head` 手验。

**Slice A 完成门槛**：`cargo clippy --all-targets` 绿；A 的验收项（envelope 结构、指标自洽、collection 子集、人类摘要）通过。可在此 ship 一次。

## Slice B：本地 server + 前端 SPA（R5–R6, R7 服务部分）

### B1 — 依赖

- `Cargo.toml`（workspace）加 `tiny_http = "<最新稳定>"`；`src/zot-cli/Cargo.toml` 加 `tiny_http.workspace = true`。
- 验证：`cargo build`。

### B2 — 前端资源

- 新建 `src/zot-cli/assets/graph/`：`index.html`、`app.js`、`cytoscape.min.js`（vendored，内联离线）。
- 前端：`fetch('/graph.json')` → Cytoscape 渲染 + 社区着色 + 搜索/年份/标签/类型/最小度过滤 + 节点详情面板。

### B3 — server 模块

- 新建 `src/zot-cli/src/commands/graph/server.rs`：`serve(graph: KnowledgeGraph, port: u16, open: bool) -> Result<()>`。
- `tiny_http::Server::http("127.0.0.1:port")`，失败回退 `127.0.0.1:0`；打印 URL；`open` 时 `util::open_target(url)`。
- accept 循环跑在阻塞线程；主任务 `tokio::signal::ctrl_c()` 触发优雅停服。
- 路由：`/`→index.html，`/app.js`/`/cytoscape.min.js`→内联资源，`/graph.json`→`serde_json::to_string(&graph)`；其余 404；非 GET → 405。
- 资源用 `include_str!`/`include_bytes!`。

### B4 — 接线 serve 子命令

- `cli/args.rs`：serve flags `--port <u16>`（默认 7878）、`--no-open`。
- `commands/graph.rs`：serve 分支构图后调 `server::serve`。
- `cli.rs` 测试：加 `["zot","graph","serve"]`、`["zot","graph","serve","--no-open","--port","7901"]`。
- 验证：`cargo test -p zot-cli`；手验 `cargo run -- graph serve --no-open --port 7901` 后 `curl -s 127.0.0.1:7901/` 与 `curl -s 127.0.0.1:7901/graph.json`，`Ctrl-C` 干净退出。

### B5 — 最终质检（全 scope）

- `python ./.trellis/scripts/get_context.py --mode packages` 列受影响包，逐包过 spec index 的 Quality Check。
- `cargo clippy --all-targets`（deny-lints 全绿，新代码无 unwrap/expect/todo!）。
- `cargo test`（全量）。

## 验证命令汇总

```bash
cargo build
cargo clippy --all-targets
cargo test
cargo run -- graph --json
cargo run -- graph serve --no-open --port 7901   # 另开终端 curl 验证后 Ctrl-C
```

## 风险与回滚点

- **B3 server 生命周期 / `tiny_http` 新依赖**：最高风险，隔离在 `commands/graph/server.rs` + assets；回滚 = 删 server 模块/assets/依赖，保留 Slice A。
- **label propagation 不确定性**：用确定性遍历 + 固定轮数，单测两次跑结果一致兜底。
- **热门标签 hairball / 性能**：`max_group_size` 封顶；必要时单测一个大组场景。
- **端口占用**：回退 `:0` 兜底。
