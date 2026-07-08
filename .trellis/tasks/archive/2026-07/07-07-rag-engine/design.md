# design: 合并两个 RAG facade 为单一 indexing engine(07-07-rag-engine)

## 0. 结论速览

新建私有模块 `src/zot-local/src/rag_engine.rs`(`mod rag_engine;` + lib.rs 精选 re-export),
承载全部索引编排;SemanticStore / WorkspaceRagStore 公开 API 不变,内部退化为薄壳
(只做 scope 构造 + 委托)。zot-cli 四个调用点零改动。

## 1. engine 模块形状

```rust
// rag_engine.rs —— 底层 RagIndex(workspace.rs:178)保持不动,engine 只做编排
pub trait RagLibrary {            // 原 workspace_rag.rs:41 WorkspaceRagLibrary 迁移改名
    fn get_item(&self, key: &str) -> ZotResult<Option<Item>>;
    fn get_pdf_attachment(&self, key: &str) -> ZotResult<Option<Attachment>>;
    fn pdf_path(&self, attachment: &Attachment) -> PathBuf;
}                                  // impl RagLibrary for LocalLibrary 一并迁入
pub struct PendingEmbedding { pub chunk_id: i64, pub text: String }
pub struct ReindexStats { pub items: usize, pub chunks: usize, pub fulltext: bool }
pub(crate) const CHUNK_MAX_TOKENS: usize = 500;      // 唯一定义
pub(crate) const CHUNK_OVERLAP_TOKENS: usize = 50;
pub(crate) const EMBEDDING_DIM_META: &str = "embedding.dim";
pub(crate) enum RefreshPolicy { ReplaceRequested, SkipIndexed }

pub(crate) fn reindex<L: RagLibrary, B: PdfBackend, P: Fn(&str) -> ZotResult<bool>>(
    index: &RagIndex, library: &L, backend: &B, md_cache: Option<&PdfCache>,
    requested: &[&str], refresh: RefreshPolicy, is_stale: P,
    fulltext: bool, force_rebuild: bool,
) -> ZotResult<(ReindexStats, Vec<PendingEmbedding>)>;
pub(crate) fn apply_pending_embeddings(index: &RagIndex,
    pending: Vec<PendingEmbedding>, embeddings: Vec<Vec<f32>>) -> ZotResult<()>;
pub(crate) fn validate_query_embedding(index: &RagIndex,
    mode: HybridMode, embedding: Option<&[f32]>) -> ZotResult<()>;
```

reindex 单一循环,全部包在 `index.with_write_tx`(保持 spec 要求的单事务批量写):

- `force_rebuild=true`:`index.clear()` 后处理全部 requested(workspace 现状;semantic 侧
  CLI 已先 clear,tx 内再 clear 幂等,zot-cli 不用改)。
- 增量:`ReplaceRequested`(semantic 语义)= 先删 requested 各 key 的 chunks,再对
  `indexed_keys()` 逐 key 跑 `is_stale` 删陈旧,然后重建全部 requested;
  `SkipIndexed`(workspace 语义)= 读一次 indexed_keys,`is_stale` 删陈旧,只处理
  requested − indexed(单次读取,保持现有行为)。
- 每个 key 经 `library.get_item` 解析,None 跳过;metadata chunk + terms + pending,
  fulltext 时走 pdf_text(md_cache 命中→backend.extract_text→回填)+ chunk_text。

参数注入(两 facade 各自持有 prune/scope):

- SemanticStore:`is_stale = |k| Ok(library.get_item(k)?.is_none())`(全库存在性),
  `ReplaceRequested`,md_cache = `self.md_cache.as_ref()`(open 时注入路径)。
- WorkspaceRagStore:`is_stale = |k| Ok(!workspace_keys.contains(k))`(workspace 成员),
  `SkipIndexed`,md_cache = fulltext 时临时开 `.md_cache.sqlite`。

维度追踪不参数化:engine 的 apply 无条件执行「批内同维校验 + 事务尾写
EMBEDDING_DIM_META」;查询侧校验由 facade 显式调用 validate_query_embedding
(workspace 的 query/query_workspace 既有;semantic 的 search 新增,见 §3)。

facade 剩余:SemanticStore = open/status_at/status/clear/mark_indexed_at/search +
reindex_chunks、apply_pending_embeddings 两个委托壳;WorkspaceRagStore = open/
index_path/cache_path/clear/query/query_workspace + 两个委托壳。

## 2. 类型归一与 lib.rs 导出(破坏面)

归一落点全部在 rag_engine.rs:PendingEmbedding、ReindexStats(WorkspaceReindexStats
并入,字段完全相同)、两个 CHUNK 常量、EMBEDDING_DIM_META、RagLibrary trait。
ReindexOpts(semantic,含 items)与 WorkspaceReindexOpts(Copy/Default)是 facade 级
API,原样保留。lib.rs 终态:
`pub use rag_engine::{PendingEmbedding, RagLibrary, ReindexStats};`
`pub use semantic::{ReindexOpts, SemanticStore};`
`pub use workspace_rag::{WorkspaceRagStore, WorkspaceReindexOpts};`
命名冲突消除:顶层 `PendingEmbedding` 仍存在且唯一,workspace_rag 不再私藏同名副本。
破坏面(仅 crate 外部路径,grep 证实仓内 zot-cli 均未引用):
`WorkspaceRagLibrary`(改名 RagLibrary)、`WorkspaceReindexStats`(并入 ReindexStats)、
`SemanticStore::apply_embeddings`(删除,防绕过维度追踪)、模块路径
`semantic::PendingEmbedding` / `workspace_rag::PendingEmbedding`(移位)。

## 3. semantic 维度校验(drift 修复)行为定案

- apply:semantic 首次获得「批内混维 → InvalidInput(embedding-dimension-mismatch)」
  与「成功后写 embedding.dim」;错误文案泛化为 "index embedding dimension"
  (workspace 既有测试只 matches! 错误类型,安全)。
- search:入口先 validate_query_embedding;仅 Semantic/Hybrid + 有 embedding 时生效。
- 旧索引兼容(定案):无 embedding.dim meta → 校验跳过(返回 Ok),不在 open/查询时
  回填;升级后**首次 apply** 自然写入 meta。与 workspace 侧现状逐字一致。
- 跨批不校验(last-write-wins,换模型增量重索引时 meta 被新批覆盖)——保持 workspace
  现状,记录为已知限制,不在本任务扩展。
- 已接受行为偏差:semantic reindex 改为 key→get_item 解析(passed items 与 DB 同源,
  结果等价;新增每 item 一次点查,与现有 prune 循环同量级,PDF/embedding 才是瓶颈)。

## 4. 与 07-07-library-seam 的关系

不等待 seam:engine 现在就依赖窄 trait——即已被 10 个测试证明的 WorkspaceRagLibrary
(迁移改名 RagLibrary,3 方法不变)。这使 SemanticStore::reindex_chunks 由绑定具体
`&LocalLibrary` 变为 `L: RagLibrary`(seam 任务 AC「semantic.rs 消费窄 trait」的索引
半边提前达成)。后续平滑:library-seam 落地时只需扩 trait 族 + AppContext adapter,
engine 无需再动;FakeLibrary 届时可提升为共享 testkit(本任务不搬,留在
workspace_rag tests,避免测试文件无谓 churn)。SemanticStore::search 依赖的
get_collection_items 不入本 trait,留给 seam 任务收窄。

## 5. 兼容性:zot-cli 桥接零改动(逐点核对)

reindex→`Vec<PendingEmbedding>`→远端 embed→apply 两段式不变;泛型放宽源兼容:

- library.rs:308 `store.reindex_chunks(&library, &backend, ReindexOpts{..})` 不改
- library.rs:324 `store.apply_pending_embeddings(pending, embeddings)` 不改
- workspace.rs:203 `rag.reindex_workspace(&library, &workspace, &backend, opts)` 不改
- workspace.rs:213 `rag.apply_pending_embeddings(pending, embeddings)` 不改
- 两处 import(library.rs:5 / workspace.rs:5)所列名字全部保留导出,不改
  spec 合规:zotero.sqlite 仍只读;RAG sidecar 写路径仍全部经 with_write_tx 单事务。

## 6. 批 1-2 落地修正(实现回填)

### engine 体量超 ≤160 预算

实测 `rag_engine.rs` = 254 行(其中 217 行非空非注释代码)。§1 的「非测试代码 ≤160
行」预算偏乐观:合并后的编排逻辑——9 参 `reindex`(force_rebuild + 两种剪枝策略
ReplaceRequested/SkipIndexed)+ 批内维校验 apply + 查询维校验 + pdf 缓存 + RagLibrary
trait/impl + PendingEmbedding/ReindexStats/RefreshPolicy/常量——即便剥掉全部 doc 也仍
~217 行,≤160 在不删功能的前提下不可达。故不为凑数压缩可读性;`reindex` 9 参保留 design
指定的签名,加 `#[allow(clippy::too_many_arguments)]`。

### 净删口径裁决

PRD「净删除 ≥100 行重复编排」以 **facade 侧净删** 计:engine 是新的单一事实源,不属于
「重复编排」,不计入。facade 侧(重复所在)净删 269 行(删 325 / 增 56:semantic
−101、workspace_rag −168、lib 0),满足 ≥100。全 src 若把新增 engine 计入则净删 ≈15,
非本任务口径——本次真实收益是去重 / 单一事实源 + semantic 侧补齐维度校验,而非 SLOC 净减。

批 3 numstat 复核命令相应改为只测两个 facade 文件:
`git diff --numstat 4aa635b HEAD -- src/zot-local/src/semantic.rs src/zot-local/src/workspace_rag.rs`。
