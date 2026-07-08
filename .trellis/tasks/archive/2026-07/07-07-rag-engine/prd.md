# 合并两个 RAG facade 为单一 indexing engine

## Goal

SemanticStore(semantic.rs)与 WorkspaceRagStore(workspace_rag.rs)在同一个 `RagIndex` 之上把索引编排复制了约 70%,且已产生真实漂移。收敛为一个以 prune 谓词 + 维度追踪参数化的 indexing engine,两个 facade 各剩薄壳。

## Problem & Evidence(2026-07-07 逐处核实)

复制对(semantic.rs ↔ workspace_rag.rs):

- `PendingEmbedding` :32 ≡ :17(逐字相同)
- `CHUNK_MAX_TOKENS=500 / CHUNK_OVERLAP_TOKENS=50` :19-20 ≡ :12-13
- reindex 循环 :135-168 ≈ :133-167
- pdf_text 缓存取用 :174 ≡ :274
- apply_pending_embeddings :203 ≈ :173

已漂移(核心动机):**embedding 维度校验只有 workspace 侧有**(`EMBEDDING_DIM_META` workspace_rag.rs:14 + validate_query_embedding:242);semantic 侧完全不追踪维度。

底层不重复(保持不动):`RagIndex`(workspace.rs:178)持有 schema(:202-220)、BM25(:507)、cosine(:568)、RRF(:985)、chunk_text(:940),各只有一份。

跨 crate seam 正确(保持不动):zot-local 不依赖 zot-remote;embedding 调用由 zot-cli 桥接(reindex→`Vec<PendingEmbedding>`→远端 embed→apply),async 网络边界在 composition root。

其它:lib.rs 只 re-export semantic 侧 `PendingEmbedding`(命名冲突泄漏)。

## Requirements

- 一个 reindex session module 承载编排:遍历 items、pdf_text 缓存、chunk、写 pending、apply embeddings
- 以参数区分两个用例:prune 谓词(全库存在性 vs workspace 成员)+ 维度追踪
- `PendingEmbedding` / 常量 / Stats 类型归一,pub 导出无命名冲突
- semantic 侧补齐维度校验(修 drift)
- reindex→apply 两段式 interface 与 zot-cli 桥接方式保持兼容

## Acceptance Criteria

- [ ] grep `PendingEmbedding` 定义 = 1 处;`CHUNK_MAX_TOKENS` 定义 = 1 处
- [ ] semantic index 具备与 workspace 侧一致的维度校验(新测试证明)
- [ ] 既有 semantic_index.rs(4 测试)与 workspace_rag(10 测试,FakeLibrary/FakeBackend)全绿
- [ ] 净删除 ≥ 100 行重复编排
- [ ] `cargo clippy` / `cargo test` 全绿

## Notes

- 复杂任务:`task.py start` 前需 design.md + implement.md。
- 独立于 cmd-output / remote-transport,可并行;若 07-07-library-seam 先行,engine 可直接依赖窄 trait(FakeLibrary 测试直接复用)。
- 父任务:07-07-arch-deepening(评审候选 C,Strong)。
