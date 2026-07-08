# implement: 07-07-rag-engine

开工前记录基线:`BASE=$(git rev-parse HEAD)`(净删行数与回滚均以此为锚)。

## 批1 engine module + 类型归一 + semantic 薄壳化

1. 新建 `src/zot-local/src/rag_engine.rs`:迁入 PendingEmbedding/ReindexStats/两常量/
   EMBEDDING_DIM_META/RagLibrary trait(+LocalLibrary impl);实现 reindex(单循环,
   RefreshPolicy + is_stale 谓词)、pdf_text、apply_pending_embeddings(批内同维校验
   - 写 dim meta)、validate_query_embedding。非测试代码预算 ≤160 行。
2. workspace_rag.rs 机械改名引用(use/泛型 bound/tests 的 FakeLibrary impl 行,
   约 5 处),trait 定义与 LocalLibrary impl 从该文件删除;编排暂不动。
3. semantic.rs 薄壳化:删本地 PendingEmbedding/ReindexStats/常量/pdf_text/
   apply_embeddings;reindex_chunks 改 `L: RagLibrary`,构造 keys+谓词后委托 engine;
   apply_pending_embeddings 委托 engine(即刻获得 dim 写入)。
4. lib.rs:`mod rag_engine;` + 导出 {PendingEmbedding, RagLibrary, ReindexStats},
   删 `semantic::{PendingEmbedding, ReindexStats}` 与 `WorkspaceRagLibrary` 导出。

验证:`cargo test -p zot-local && cargo clippy -p zot-local --all-targets -- -D warnings`
(semantic_index.rs 4 测 + workspace_rag 10 测全绿);`cargo check --workspace`。
回滚点:commit `refactor(local): rag engine 抽出 + semantic 薄壳化`;失败 reset 到 BASE。

## 批2 workspace 薄壳化 + 维度校验统一 + lib.rs 终态

1. workspace_rag.rs:reindex_workspace 构造 workspace_keys+谓词(SkipIndexed)委托
   engine;apply_pending_embeddings/validate_query_embedding/pdf_text 本地实现删除,
   改调 engine;删 WorkspaceReindexStats(返回 engine::ReindexStats)。
2. semantic.rs:search 入口加 `validate_query_embedding(&self.index, mode, embedding)?`。
3. lib.rs 终态核对(§2 三行导出);更新受影响 doc 注释。

验证:批1 命令全套;再加唯一性 grep——
`grep -rn "struct PendingEmbedding" src/` = 1 处;
`grep -rn "CHUNK_MAX_TOKENS" src/zot-local/src` 定义 1 处(引用仅 engine 内);
`grep -rn "EMBEDDING_DIM_META" src/zot-local/src` 定义 1 处。
回滚点:commit `refactor(local): workspace_rag 薄壳化 + 维度校验统一`;可单独 revert
(批1 后 workspace_rag 独立可编译)。

## 批3 semantic 维度校验新测试 + 净删行数核对

1. tests/semantic_index.rs 增 4 测(复用 search_regression.rs 的 fixture 开库模式):
   a. apply 后 `RagIndex::open(同路径).get_meta("embedding.dim") == Some("3")`;
   b. apply dim3 后 search(Semantic, dim2 query)→ InvalidInput;
   c. 无 meta 旧索引 search(Semantic, 任意维)→ Ok(校验跳过,兼容定案);
   d. apply 批内混维([dim3, dim2])→ InvalidInput。
2. 净删核对:`git diff --numstat $BASE -- src/zot-local/src`(不含 tests/),
   删−增 ≥ 100;不足则按预算收紧 engine(内联 helper、精简 doc)。

验证:`cargo test --workspace && just ci`(fmt+check+clippy+test 全绿;注:仓库有
预存 fmt 漂移,just ci 的 fmt gate 若仅因预存漂移红灯,以漂移清单不扩大为准)。
回滚点:commit `test(local): semantic 维度校验回归测试`。

## 验收对照(PRD 5 条)

1. PendingEmbedding / CHUNK_MAX_TOKENS 定义各 1 处 → 批2 grep 输出存档
2. semantic 与 workspace 维度校验一致(新测试证明)→ 批3 测试 a–d
3. semantic_index.rs 4 测 + workspace_rag 10 测全绿 → 每批 cargo test
4. 净删 ≥100 行重复编排 → 批3 numstat(engine ≤160 行 guardrail)
5. cargo clippy / cargo test 全绿 → 每批 + 批3 just ci

## 回滚策略

每批一个独立 commit,主干任意批后均可编译发布(zot-cli 全程零改动);
任一批红灯:`git revert <该批 commit>` 或 reset,批间无跨批未完成状态。
