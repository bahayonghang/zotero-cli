# library seam 设计稿 v2(已定案)

> 目标:AppContext 从「藏构造」升级为「可替换行为」。seam 位置不动(仍 context-mediated,
> database-guidelines.md 语义不变),只收窄消费者依赖。
> **状态:2026-07-08 用户拍板「全按推荐」,D1-D6 定案见文末;可 task.py start。**

## 0. 事实基线(2026-07-08 复核)

- `context.rs:38/:42` 返回具体 `LocalLibrary`/`ZoteroRemote`;`db.rs` 40 pub fn ≈ 10 职责簇
- 范本:`workspace_rag.rs:41` `WorkspaceRagLibrary`(3 方法)+ FakeLibrary(:302),10 测试在用
- `semantic.rs:114`(reindex_chunks)/:231(search)硬绑 `&LocalLibrary`
- `PdfiumBackend` 构造 8 处:doctor.rs:27、library.rs:301、annotation.rs:90/:141、
  workspace.rs:196、read.rs:71/:192、write.rs:234(+6 import 行 = 14 处/6 文件)
- `util.rs:110-133` require_item/require_item_pdf/require_pdf_attachment 也绑具体类型
- 注意:rag-engine 任务并行在建 zot-local/rag_engine.rs,W2 需协调(见 §5)

## 1. 窄 trait 族切分(首轮 5+1 个,均由 LocalLibrary 薄委托实现)

按**数据域**切(方案 A,推荐),trait 定义放 zot-local 新文件 `library_traits.rs`
(semantic.rs 消费方在 zot-local 内;pdf_path 是本地文件系统语义,不上提 zot-core):

| trait                                               | 方法(签名与 db.rs 现状同形)                                                               | 消费者                                                                           |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `ItemReader`                                        | get_item / list_items / search / get_recent_items / get_recent_items_by_count             | library.rs、workspace.rs、sync.rs、item/tag.rs、util::require_item               |
| `CollectionNav`                                     | get_collections / get_collection / get_subcollections / search_collections                | collection.rs 读侧                                                               |
| `CollectionContent`                                 | get_collection_items / get_collection_item_count / get_collection_tags                    | collection.rs、library.rs:348、workspace.rs:85、scite.rs:114、semantic.rs search |
| `NoteReader`                                        | get_notes / search_notes                                                                  | item/note.rs、item/read.rs:21                                                    |
| `AttachmentSource`                                  | get_attachments / get_attachment_by_key / get_pdf_attachment / attachment_path / pdf_path | item/read.rs、item/annotation.rs、util::require_*                                |
| `PdfSource`(引擎侧,= 现 WorkspaceRagLibrary 3 方法) | get_item / get_pdf_attachment / pdf_path                                                  | workspace_rag.rs、semantic.rs reindex、(rag_engine 在制品)                       |

- 与 AttachmentSource 的方法重叠是**有意的**:接口重叠≠实现重复,LocalLibrary 同一 inherent fn 被两个窄面委托。
- 剩余簇(本轮不建 trait,消费到了再切):TagReader(get_tags)、AnnotationReader(get/search_annotations)、
  ChildReader(get_item(s)_children)、InsightReader(duplicates/related/graph/stats/trash)、
  CitationExporter(citekey/export_citation)、CatalogReader(libraries/feeds/feed_items/schema)、arxiv(sync 专用)。
- 【D1|粒度】数据域(上表)vs 命令域(每 handler 一 trait)。反例:命令域下 library.rs 一个 trait 需 12+ 方法,
  等于把「杂物抽屉命令」固化为宽接口;数据域 fake 可跨 handler 复用。推荐 A。
- 【D2|范围】首轮仅切上表 19 个热方法 vs 一次切满 40。推荐前者:无消费者的 trait 是死接口,后续波次按需提升。

## 2. sync/async 与分发方式

约束:LocalLibrary 是同步 rusqlite(Connection: Send + !Sync,含 OnceCell 缓存),每命令临时 open、
用后即弃;PDF 重活经 util.rs:6 `run_pdf`(spawn_blocking,要求 move 值 Send + 'static);AppContext derive(Debug, Clone)。

| 选项                            | 评价                                                                                                     |
| ------------------------------- | -------------------------------------------------------------------------------------------------------- |
| A. Box<dyn Trait> 存 AppContext | 破坏 Clone/Debug;把「每命令一连接」改成长驻状态,动了 seam 语义,违背 PRD                                  |
| B. 泛型 `&impl Trait` 下传      | 与 semantic/workspace_rag 既有签名同构;单态化零开销;fake 从函数签名注入                                  |
| C. 混合(推荐)                   | **库依赖用 B**(seam 在函数签名,AppContext::local_library 返回类型不变);**PdfBackend 用 Arc<dyn>**(见 §3) |

- trait 保持同步,不引 async_trait;泛型值整体 move 进 run_pdf 闭包(workspace.rs:202 已有先例)。
- 【D3|分发】推荐 C。若偏好「AppContext 全面持 trait object」,需先接受 Clone/Debug 手写与连接生命周期变化。
- 【D4|remote 侧】推荐**本轮不做**(PRD 允许):ZoteroRemote 全 async,dyn 化需 async_trait 或 AFIT+Box 权衡,
  且写路径 fake 收益集中在 merge/批量 tag 少数编排点。建议另立任务,届时优先 `#[async_trait]` + 按写用例切窄面。

## 3. AppContext adapter 形状与 Pdfium 收敛

- `local_library()`:签名不变(返回具体 LocalLibrary)。可替换性来自 handler 内层函数收窄为 `&impl XxxReader`。
- `pdf_backend()`:AppContext 新增字段 `pdf: Arc<dyn PdfBackend + Send + Sync>`(默认 PdfiumBackend,
  测试经构造注入 fake;Debug 手写或 newtype)。PdfBackend 已 object-safe(pdf.rs:61,全 &self、具体参数);
  加 blanket impl `impl<T: PdfBackend + ?Sized> PdfBackend for Arc<T>`,既有泛型消费点(reindex_chunks<B>)零改动。
- 收敛路径:8 处构造 → 全部改 `ctx.pdf_backend()`;6 处 import 随之消失。
  【D5|doctor】doctor.rs:27 依赖 `PdfiumBackend::status()`(不在 trait 上,是 Pdfium 特有诊断)。
  甲:doctor 保留具体构造(总构造数 = context 1 + doctor 1 = 2,满足 AC ≤2);
  乙:status() 入 trait(fake 返回假可用性),构造仅剩 context 1 处。推荐甲(诊断命令本就该看真环境)。
- store 构造:`SemanticStore::open(path, cache)` 已是参数注入形状,无需 trait;新增
  `AppContext::semantic_store()` 收敛 library.rs:290-297 的 md-cache 路径拼装,顺带消除与
  workspace 侧(store.root()/.md_cache.sqlite)的路径分叉。WorkspaceStore::new(None) 同理留参数注入。

## 4. 首批 fake 测试 handler 提名(AC1)

1. **commands/collection.rs 读侧**(List/Get/Subcollections/Items/Search/ItemCount/Tags 共 7 arm):
   拆内层 `fn run_read(ctx, lib: &impl CollectionNav + CollectionContent, cmd) -> Result<CommandOutput>`,
   写 arm 留在外层走 ctx.remote()。断言面:① List/Items/Tags 的 JSON envelope(as_json() 已有 test 口);
   ② Get 未命中 → 错误码 `collection-not-found`(collection.rs:20-24 稳定契约);③ ItemCount payload
   形状 {collection_key, item_count};④ 非 json 模式 as_json()==None。
2. **commands/item/note.rs 读侧**(List/Search 2 arm):最小 NoteReader 面。断言面:① List envelope
   含 note key/content;② Search 的 limit 参数确实传抵 fake(交互断言);③ fake 返回 Err 时错误透传。

- 备选(第三个):item/read.rs handle_get(item+notes+attachments 聚合 payload,回归价值最高但 fake 面稍宽)。
- ctx 构造无障碍:tests 已有手工 AppContext 先例(output.rs:87、workspace.rs:251)。

## 5. 迁移波次概要

- **W0 地基**:zot-local 建 library_traits.rs(trait + LocalLibrary impl + Arc blanket impl);不动 db.rs 内部。
- **W1 Pdfium 收敛**:AppContext 加 pdf 字段与 pdf_backend();替换 8 构造点;doctor 按 D5。grep 验收 AC3。
- **W2 semantic 对等**(AC2):semantic.rs:114 → `L: PdfSource`、:231 → `L: ItemReader + CollectionContent`;
  补 fake 测试对齐 workspace_rag。⚠ 与 rag-engine 在制品同文件域,开工前 rebase 并协调:
  若 rag_engine.rs 已定义等价 RagLibrary trait,semantic.rs 直接复用之,不另造 PdfSource。
- **W3 handler fake 测试**(AC1):collection.rs / note.rs 内层化 + 测试;util::require_* 泛型化。
- **W4 收尾**:semantic_store() 路径收敛;clippy/test 全绿;grep 复核;更新 database-guidelines.md 一节
  (「窄 trait 消费」补充,不改 access boundary)。

## 决策点清单(2026-07-08 定案:全按推荐)

- D1 ✅ trait 粒度:数据域
- D2 ✅ 首轮范围:仅 19 个热方法(5 个新 trait;第 6 个见 D6 事实更新)
- D3 ✅ 分发:混合——库走泛型下传、PdfBackend 走 Arc<dyn> 入 AppContext
- D4 ✅ ZoteroRemote 本轮不 trait 化,另立任务
- D5 ✅ doctor 保留 1 处具体 PdfiumBackend(总构造 2 处:context + doctor)
- D6 ✅(被现实消解)rag-engine 已落地:workspace_rag 的 `WorkspaceRagLibrary` 已在该任务中
  统一为 `rag_engine.rs:26` 的 `RagLibrary`(3 方法,即本稿的 PdfSource),semantic.rs
  `reindex_chunks` 也已泛型化消费之。**不再新造 PdfSource,直接复用 RagLibrary。**

## 定案后事实刷新(2026-07-08,rag-engine 落地导致)

- §0 中「rag_engine 在制品」已成事实:`RagLibrary` 存在且被 workspace_rag(:50/:141 FakeLibrary)
  与 semantic(:94 reindex_chunks)消费——W2 的 reindex 半边已完成。
- W2 剩余工作收窄为:`semantic.rs:126 search` 仍绑 `&LocalLibrary`(用 get_collection_items +
  get_item)→ 改 `L: ItemReader + CollectionContent`,并补 fake 测试。
- 首轮新建 trait 数:5(ItemReader/CollectionNav/CollectionContent/NoteReader/AttachmentSource)。
