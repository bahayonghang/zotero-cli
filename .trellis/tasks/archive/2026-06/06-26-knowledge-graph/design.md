# Design — 本地 Zotero 知识图谱与可视化分析

## 1. 架构与边界

纯增量、对 `zotero.sqlite` 只读、不改任何现有命令/schema。三层落位：

| 层          | 新增内容                                                                                                                                      | 依赖原则                              |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| `zot-core`  | `model.rs` 新增图类型 + `lib.rs` 重导出                                                                                                       | 仅 serde，**零新依赖**                |
| `zot-local` | 新模块 `src/graph.rs`（构图 + 算法）；`lib.rs` 导出                                                                                           | 复用现有 `LocalLibrary`，**零新依赖** |
| `zot-cli`   | `commands/graph.rs`（命令）+ `commands/graph/server.rs`（HTTP）+ `assets/`（前端）；`cli.rs`/`cli/args.rs`/`commands/mod.rs`/`format.rs` 接线 | 新增 `tiny_http` 依赖                 |

## 2. 数据契约（zot-core/model.rs）

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeRelation { Coauthor, Tag, Collection, Related }

pub struct GraphNode {
    pub key: String,
    pub title: String,
    pub item_type: String,
    pub year: Option<String>,
    pub first_author: Option<String>,
    pub degree: usize,
    pub weighted_degree: i64,
    pub community: usize,
}

pub struct GraphEdge {
    pub source: String,            // item key
    pub target: String,            // item key，约定 source < target 以去重
    pub weight: i64,
    pub relations: Vec<EdgeRelation>, // 该对节点命中的关系类型
}

pub struct CommunitySummary { pub id: usize, pub size: usize, pub top_tags: Vec<String> }
pub struct RankedNode { pub key: String, pub title: String, pub score: i64 }

pub struct GraphMetrics {
    pub node_count: usize,
    pub edge_count: usize,
    pub connected_components: usize,
    pub top_by_degree: Vec<RankedNode>,
    pub top_by_weighted_degree: Vec<RankedNode>,
    pub communities: Vec<CommunitySummary>,
    pub top_authors: Vec<TagSummary>,   // 复用现有 TagSummary{name,count}
    pub top_tags: Vec<TagSummary>,
}

pub struct KnowledgeGraph {
    pub scope: String,              // "library:user" / "collection:KEY"
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub metrics: GraphMetrics,
}
```

`GraphOptions`（构图入参）也放 zot-core，便于 cli→local 传递：

```rust
pub struct GraphOptions {
    pub collection: Option<String>,
    pub min_shared_tags: usize,   // 默认 2
    pub max_group_size: usize,    // 默认 50，热门标签/大 collection 上限
    pub top_n: usize,             // 默认 20，各 Top 榜单长度
    pub relations: GraphRelationToggles, // 四类边开关，默认全开
}
```

权重默认（沿用 `get_related_items` 哲学，可作常量集中放置）：`related = 100`、`coauthor = 8`、`tag = 5 × 共享标签数`、`collection = 1 × 共享 collection 数`。同对节点多关系 → `weight` 取各项之和，`relations` 收集命中类型。

## 3. 数据流

```
CLI graph / graph serve
  └─ AppContext::local_library()                      // 现有
       └─ LocalLibrary::build_knowledge_graph(&GraphOptions) -> KnowledgeGraph   // 新
            ├─ 取节点：search(empty query, collection?, limit=MAX) → Vec<Item>   // 复用，已批量
            ├─ 建倒排：author→[key], tag→[key], collection→[key]
            ├─ 生成成对边（跳过 size>max_group_size 的组），合并同对权重/关系
            ├─ 取显式相关：一条 itemRelations 批量查询（predicateID=1，限本 scope 节点）  // 新增 SQL
            └─ 算指标：degree/weighted_degree、并查集连通分量、标签传播社区、Top 榜
  → 分支：
     --json      → print_enveloped(ctx, &graph)
     人类可读     → format::print_graph_summary(&graph.metrics)
     serve       → server::serve(graph, port, open)   // 注入 SPA
```

### 取显式相关边（新增 SQL，单次批量）

按 scope 内 itemID 集合，查 `SELECT itemID, object FROM itemRelations WHERE predicateID = 1 AND itemID IN (...)`，`object` 尾段为相关 item 的 key，过滤出两端都在节点集合内的边。放在 `graph.rs`，遵守 zot-local 数据库规范（`prepare_cached` + `sql_err` 映射）。

## 4. 图算法（手写，零依赖，确定性）

- **degree / weighted_degree**：遍历边累加。
- **连通分量**：并查集（union-find），返回分量数。
- **社区**：标签传播（label propagation）。**确定性要求**——workspace 禁 `Math.random` 式不确定；按 `key` 排序后的固定节点序迭代、平票时取最小社区 id，固定迭代轮数上限（如 20）。保证同输入同输出，便于单测。
- **代表标签**：每社区内统计成员 `tags` 频次取 Top。
- 复杂度：构边 = Σ(组内 C(k,2))，由 `max_group_size` 封顶；算法均为 O(V+E) 级，几千节点无压力。

## 5. HTTP 服务（zot-cli/commands/graph/server.rs）

- 选型：**`tiny_http`**（同步、极小、稳定）。理由：MVP 为「启动时一次性构图 + 静态 serve」，只需 3~4 个 GET 路由；在 `unwrap_used = deny` 下手写 TcpListener 解析 HTTP 风险高，axum 对纯静态又偏重。`tiny_http` 是最小可靠面。（评审若否决，可回退到极小 tokio TcpListener 实现。）
- 在 `spawn_blocking` 或独立线程跑阻塞 accept 循环（util.rs 已有 `spawn_blocking` 先例）；主线程 `tokio::signal::ctrl_c()` 等待退出信号后停服。
- 仅绑定 `127.0.0.1`。端口：先试 `--port`（默认 7878），`bind` 失败回退 `127.0.0.1:0` 由系统选空闲端口；打印最终 URL。
- 路由：`GET /` → index.html；`GET /app.js`、`GET /cytoscape.min.js` → 内联资源；`GET /graph.json` → 序列化的 `KnowledgeGraph`。其余 404。
- 资源经 `include_str!` / `include_bytes!` 编译进二进制（离线、单文件分发）。

## 6. 前端（assets/，Cytoscape.js 内联）

- `index.html` + `app.js`：`fetch('/graph.json')` 载入数据，Cytoscape 渲染（`cose`/`fcose` 力导向），社区映射颜色；左侧控制面板（搜索、年份/标签/类型/最小度过滤），右侧节点详情面板。
- 纯客户端过滤（增删可见元素），不回服务端。无外链、无 CDN。

## 7. 关键取舍

- **server vs 静态 HTML**：用户明确选 server；MVP 取「一次性构图 + 静态 serve」最小化工作量，保留 Phase 2 升级为实时 API 的空间。
- **tiny_http vs axum vs 手写**：见 §5，取 tiny_http。
- **petgraph 暂不引入**：基础指标手写即可满足 R4；介数/Louvain 留 Phase 2。
- **边模型**：以「论文-论文」单一节点类型 + 多关系合并边，规避多类型图的可视化/算法复杂度（用户已选「论文为节点」）。

## 8. 兼容性 / 回滚

- 全部新增，无现有行为变更；`graph` 命令失败不影响其它命令。
- 分两片交付，各自独立可回滚：**Slice A**（R1–R4 构图+`--json`+人类摘要）先行，可单独 ship；**Slice B**（R5–R6 serve+SPA，含 `tiny_http` 依赖）随后。最危险点是 Slice B 的服务生命周期与新依赖，隔离在独立模块，回滚只需移除 server 模块与依赖、保留 Slice A。

## 9. 安全

- 服务仅 `127.0.0.1`，不监听外网；只读本地库；不接收写请求（非 GET 一律 404/405）。无鉴权需求（本机自用）。
