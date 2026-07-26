# P2: 本地 DB 语义与查询性能

## Goal

修复审计确认的本地 SQLite 查询语义和大库退化问题：普通检索默认不再返回回收站条目，collection 名称歧义必须 fail closed，notes 不再逐条加载标签，search 只 hydrate 当前页，duplicate 和 graph 必须有显式计算预算与截断证据，并把重型同步本地查询移出 Tokio worker。

## Background

- 父任务映射：`.trellis/tasks/07-26-audit-remediation/prd.md:30` 的 `07-26-fix-db-semantics-perf`。
- 源证据：`zotero-cli-code-audit-2026-07-25.md:123`、`:142-146`、`:151` 和 `:572-634`。
- `SearchOptions::default().exclude_trashed=false`；search/list 的基础分支和过滤分支仅在调用方主动开启时排除 `deletedItems`，stats 的五类聚合也未排除回收站条目（`src/zot-local/src/db.rs:75-138`、`:1380-1460`）。
- `resolve_collection_id` 使用 `key = ? OR collectionName = ?` 的单行查询；同名 collection 会不确定选择（`src/zot-local/src/db.rs:1549-1562`）。
- `get_notes` 已一次读取 note 行，但每条 note 再调用 `get_item_tags`（`src/zot-local/src/db.rs:338-376`）。
- item hydration 已有 500 条分块的 fields/creators/tags/collections 批量 loader；缺陷在于 search 先收集、过滤、hydrate 和排序所有候选，最后才 `skip/take`（`src/zot-local/src/db.rs:225-320`、`:1781-2025`）。
- `find_duplicates` 先用固定 `limit=10_000` 加载完整 item，再对 title 做全局两层 Levenshtein；输出只保留 groups，无法说明扫描量、候选量或预算截断（`src/zot-local/src/db.rs:1150-1225`）。
- graph 对全库调用 `list_items(..., usize::MAX, 0)`，纯组装器为每个未超 `max_group_size` 的 group 展开全部 pair；多个中型 group 仍可制造无界 candidate edge（`src/zot-local/src/db.rs:1316-1337`，`src/zot-local/src/graph.rs:55-218`）。
- `zot-cli` 的 async handlers 直接打开 `LocalLibrary` 并运行 rusqlite；只有 PDF 路径已有 `run_pdf/spawn_blocking`（`src/zot-cli/src/util.rs:6-18`，`commands/library.rs:19-220`，`commands/graph.rs:17-31`，`commands/item/annotation.rs:11-37`）。
- 父任务范围外明确包含完整 Application/use-case 层、拆分 `zot-local` god object 和长期 LocalExecutor 架构；本任务只增加窄的阻塞执行 helper 和查询契约，不做模块拆分。

## Requirements

### R1: 默认排除回收站并显式恢复旧行为

- `SearchOptions::default()` 默认排除 `deletedItems`；library search/list/stats 增加 `--include-trashed`，未传时 search/list/stats、collection/workspace 读路径和 duplicate/graph 均排除回收站。
- `--include-trashed` 只作用于显式支持它的 library search/list/stats，不改变 `item deleted` 的专用回收站命令。
- search/list/stats 的 JSON envelope `meta.trash_policy` 必须稳定为 `excluded` 或 `included`；其他命令不输出该可选字段。

### R2: Collection key 优先且名称歧义 fail closed

- 先按当前 library 的精确 key 查询；命中即返回，不再与 display name 竞争。
- key 未命中时按名称收集确定性排序的候选：0 个返回 `collection-not-found`，1 个返回该 ID，多于 1 个返回 `collection-ambiguous`，hint 列出候选 keys。
- 所有通过 `resolve_collection_id` 的 search、workspace、graph 和 collection 入口继承该契约。

### R3: Notes 标签批量加载

- `get_notes` 先收集 note rows 和 item IDs，再复用已有 `load_item_tags_batch` 一次批量获取标签。
- note 顺序、HTML-to-text、parent key 和 tag 排序保持现有行为。

### R4: Search SQL 分页和 page-only hydration

- 以静态 SQL 片段和参数绑定构建统一候选条件；query 的 field/creator/tag/fulltext 匹配保持 OR 语义，collection/type/tag/creator/year 保持 AND 语义并继续转义 `LIKE`。
- 用独立 `COUNT(*)` 得到 total；page ID 查询下推 `ORDER BY/LIMIT/OFFSET`，只把 page IDs 交给现有 batch hydration。
- title、creator、dateAdded、dateModified 和 key 默认排序必须确定性；相同主排序值以 key 打破平局。
- 不引入 FTS schema 或修改 Zotero-owned 数据库索引。

### R5: 有预算且可审计的 duplicate scan

- 删除固定 10,000 item 截断；用最小 candidate projection 扫描当前 scope 的全部非回收站主条目。
- DOI exact 使用分组；title 先按 normalized 12 字符前缀、年份和首位作者 surname 生成确定性 block，只在共享 block 内计算 Levenshtein。
- library duplicates/dedupe 增加正整数 `--candidate-budget`，默认 250,000；结果输出 groups、`scanned_count`、`candidate_pair_count`、`skipped_oversize_blocks`、threshold、budget 和 `truncated`。
- 达到 budget 时停止接纳新 pair 并返回 `truncated=true`；只读 duplicates 可返回有证据的部分结果，dedupe preview/apply 必须在任何写前以 `duplicate-scan-truncated` fail closed。
- `--limit` 仍只限制返回/计划的 group 数量，不再限制扫描输入。

### R6: Graph edge budget 和截断元数据

- `GraphOptions` 增加正整数 candidate edge budget，默认 100,000；`graph` 和 `graph serve` 暴露 `--edge-budget`。
- pure assembler 按稳定 group/pair 顺序累积信号；预算只限制新的唯一 candidate pair，已有 pair 仍可接收后续关系信号。
- 超过 `max_group_size` 的 group 继续跳过并计数；超过 edge budget 的新 pair 被跳过并设置 `truncated=true`。
- `KnowledgeGraph` 增加 build 元数据：budget、candidate pair count、skipped oversize group count 和 truncated；human summary 在截断时显示明确警告。
- graph 仍输出完整 scope node 集；不增加 sampling、hub nodes 或新的数据库 schema。

### R7: 重查询离开 Tokio worker

- 在 CLI 层增加与 `run_pdf` 同级的窄 `run_local` helper：在 `spawn_blocking` 内打开 task-owned `LocalLibrary`、执行闭包并把 join failure 映射为稳定 `local-task-join`。
- library search/list/stats/duplicates/dedupe planning、graph build/serve、annotation list/search 和 workspace 的重 search/import membership 查询使用该边界。
- 不引入持久 executor、actor、线程池配置或 use-case abstraction。

### R8: 性能与兼容性回归证据

- fixture/golden tests 证明 query/filter/sort/pagination 和 duplicate/graph 小数据结果保持确定性。
- synthetic 10k/50k 规模测试断言 title candidate comparisons 和 graph candidate pairs 受预算/blocks 约束；不使用易抖动的墙钟阈值作为 CI 成败条件。
- 更新 `docs/agents/limits.md` 和适用的 Trellis spec，记录 trash、search page hydration、duplicate/graph budget 与 blocking boundary。

## Acceptance Criteria

- [x] AC1: 默认 search/list/stats 不含 deleted item；三个命令的 `--include-trashed` 恢复旧结果，JSON `meta.trash_policy` 与实际行为一致。
- [x] AC2: collection key 与同名 name 冲突时 key 优先；同名多 collection 返回 `collection-ambiguous` 并列出稳定候选 keys。
- [x] AC3: `get_notes` 复用 batch tag loader，结果内容、顺序和标签排序不回归。
- [x] AC4: search 的 total、OR/AND filters、LIKE literal 语义、四种排序和 offset/limit golden tests 通过；实现只 hydrate page IDs。
- [x] AC5: duplicate scan 不含 10,000 silent cap，输出完整 scan 元数据；DOI exact 与 title blocking 的 golden groups 正确，budget 截断可见且 dedupe fail closed。
- [x] AC6: graph candidate pairs 不超过 edge budget，oversize groups 与预算截断均有结构化元数据，未截断小 fixture 的 nodes/edges/metrics 保持一致。
- [x] AC7: `run_local` 在 single-thread Tokio 测试中证明工作运行于 blocking thread；指定 library/graph/annotation/workspace 路径不再直接执行重 rusqlite 查询。
- [x] AC8: 10k/50k synthetic complexity tests 通过且不依赖墙钟阈值。
- [x] AC9: CLI parse、JSON/human output、dedupe safety、zot-local fixture 回归和 specs/docs 同步完成。
- [x] AC10: 聚焦测试、`cargo test -p zot-local`、`cargo test -p zot-cli` 与最终 `just ci` 全部通过。

## Out Of Scope

- 不拆分 `src/zot-local/src/db.rs`，不新增 Application/use-case 层、持久 LocalExecutor、actor 或 daemon runtime。
- 不修改 Zotero-owned schema/index，不引入 FTS5、ANN、trigram/minhash 依赖或 Criterion 生产依赖。
- 不实现 duplicate continuation cursor、top-k graph neighbor、hub/bipartite 节点、graph sampling 或缓存。
- 不处理 PDF cache、remote HTTP、附件写入、graph viewer URL、sidecar 并发等后续子任务。
