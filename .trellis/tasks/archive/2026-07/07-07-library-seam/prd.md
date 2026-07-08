# library seam 可替换化(窄 trait + AppContext)

## Goal

把「context-mediated access」从**藏构造**升级为**可替换行为**:AppContext 交出可替换 adapter,消费者依赖按用例切分的窄 trait 而非 40 方法的具体类型;解锁 handler 级测试。seam 位置不动,只让它真正可替换。

## Problem & Evidence(2026-07-07 核实)

- `context.rs:38 / :42` 返回具体 `LocalLibrary` / `ZoteroRemote`;测试无法替换假实现——唯一的 AppContext 测试靠在 `ctx.remote()` 之前短路才通过(library.rs:294-301)
- zot-cli **9 个 handler 文件零测试**;`ctx.local_library()?` / `ctx.remote()?` 之后的一切(全部读输出、全部写路径)只能靠真 sqlite + 真网络验证
- `db.rs` 恰好 **40 个 pub fn**(grep 复核),约 10 类职责;semantic.rs:114/:231 绑定具体 `&LocalLibrary`
- **seam 可行性已有实证**:workspace_rag.rs:41 的 `WorkspaceRagLibrary` trait(3 方法)+ FakeLibrary(:334)已被 10 个测试使用——这不是假设的 seam,是已被证明却只用了一次的 seam
- `PdfiumBackend` 硬编码 **14 处 / 6 文件**(grep 复核);PDF 缓存路径处理已分叉(library.rs:341 vs workspace.rs:217)
- spec 约束:`.trellis/spec/zot-cli/backend/database-guidelines.md` context-mediated access——本任务强化该决定,不改变它

## Requirements

- 按用例切窄 trait 族(每个 3-5 方法量级),`LocalLibrary` 实现之;`WorkspaceRagLibrary` 模式为范本
- AppContext 提供可替换 adapter(含 pdf backend 与 store 构造),测试可注入 fake
- 不搬 SQL、不拆 db.rs 内部实现(那是另一个决定);只收窄消费者依赖的 interface
- `ZoteroRemote` 侧是否同轮 trait 化在 design.md 定夺(async trait 形状需要设计对话)

## Acceptance Criteria

- [ ] 至少 2 个此前不可测的 handler 流经 fake adapter 获得测试
- [ ] semantic.rs 消费窄 trait,可用 fake 测试(与 workspace_rag 对等)
- [ ] grep `PdfiumBackend` 构造 ≤ 2 处(集中于 context / composition root)
- [ ] 既有测试全绿;context-mediated access 语义不变
- [ ] `cargo clippy` / `cargo test` 全绿

## Notes

- 复杂任务:`task.py start` 前需 design.md(trait 切分粒度、sync/async、`Box<dyn>` vs 泛型)+ implement.md。
- 顺序:建议在 07-07-cmd-output 之后(handler 先变「返回数据」形状,fake 测试才有断言面)。
- Worth exploring 级:trait 形状需要设计对话,勿机械动手。父任务:07-07-arch-deepening(评审候选 D)。
