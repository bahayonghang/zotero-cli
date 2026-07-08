# 架构加深优化:收敛半成品 seam 与重复模块

## Goal

落实 2026-07-07 架构评审的全部发现(6 个加深候选 + 4 个小信号)。评审方法:两个只读探查代理全库扫描 + 关键计数 grep 独立复核(85 / 14 / 6 / 40 均已证实)。目标是 testability 与 AI-navigability。

贯穿性根因:**多处 seam 修到即将产生 leverage 的位置就停了**——HttpRuntime 只共享连接池;base URL 只有一半 client 可覆写;`WorkspaceRagLibrary` trait 证明 fake adapter 可行却只有一个模块使用。库内已有两个 deep module 范本可效仿:`item/merge.rs`(纯 plan 函数 + 测试)与 `graph.rs`(一个入口藏 ~400 行确定性算法)。

## Requirements(任务地图)

| 子任务 | 问题 | 优先级 |
|---|---|---|
| 07-07-cmd-output | 85 处 `if ctx.json` 输出分支;workspace export 绕过 envelope 契约 | P1 |
| 07-07-remote-transport | `remote_err` ×6;响应处理仅 zotero.rs 集中;base URL 2 处硬编码;HTTP 边界零测试 | P1 |
| 07-07-rag-engine | semantic.rs 与 workspace_rag.rs ~70% 复制,维度校验已漂移 | P1 |
| 07-07-library-seam | AppContext 交出具体类型,9 个 handler 零测试;db.rs 40 方法宽 interface | P2 |
| 07-07-pure-plan | write/sync/scite 纯逻辑被困 I/O 夹层,零测试 | P2 |
| 07-07-related-scorer | graph.rs 与 db.rs 相关性权重分叉,两命令排名不一致 | P2 |
| 07-07-envelope-err | ErrorPayload ≡ EnvelopeError 双胞胎 | P3 |
| 07-07-error-helpers | 边界错误手写 63 处,item-not-found ×6 | P3 |
| 07-07-config-apply | config.rs root/profile 四函数近重复 | P3 |
| 07-07-pdf-http | pdf.rs reqwest::blocking 第二 HTTP 栈 | P3 |

推荐顺序(写入各子任务 prd.md,非硬依赖):

1. `envelope-err` 先行(小;cmd-output 的输出模块会用到 `err()` 构造器)
2. `cmd-output` 是基石:handler interface 从「怎么打印」翻转为「返回什么」
3. `pure-plan`、`library-seam` 依赖 cmd-output 的形状
4. `remote-transport`、`rag-engine`、`related-scorer` 相互独立,可并行;`pdf-http` 与 remote-transport 同期评估
5. 其余 P3 随时可做

## Acceptance Criteria(跨子任务)

- [ ] 全部 10 个子任务归档,或以 ADR/spec 形式记录「不做」的承重理由
- [ ] workspace `cargo test` 全绿;新增 handler 级 / transport 级 / plan 级测试
- [ ] grep 复核:`if ctx.json` ≤ 2 处;`fn remote_err` 定义 = 1;`PendingEmbedding` 定义 = 1;`PdfiumBackend` 构造 ≤ 2
- [ ] `zot related` 与 `zot graph` 对同一条目对给出一致相对排名
- [ ] 所有命令在 `--json` 下输出 envelope(或 spec 记录豁免)
- [ ] 不违背既有 spec 决定:zotero.sqlite 只读、graph.rs 不开 SQLite、zot-remote 无持久化、库代码不打印、错误码对外稳定
- [ ] 父任务收尾:全范围质量检查 + 更新 .trellis/spec 相关文档(输出契约、transport 模式、seam 约定)

## Notes

- 复杂子任务(cmd-output / remote-transport / rag-engine / library-seam)在 `task.py start` 前需补 design.md + implement.md;P3 子任务 PRD-only 即可。
- 词汇约定沿用评审:module / interface / seam / adapter / depth / leverage / locality。
- 证据 file:line 均为 2026-07-07 快照;实施前如代码已变动,先复核行号。
- 父任务不承载直接实现;它持有需求集、顺序建议与最终集成审查。
