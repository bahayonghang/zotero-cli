# 收敛 85 处输出分支为 CommandOutput 模块

## Goal

把 JSON-vs-human 的输出决策从 13 个命令文件中的 85 处 `if ctx.json` 分支收敛为一个 module(dispatch 层唯一一次分支);顺带处理 workspace export 在 `--json` 下绕过 envelope 契约的问题。这是「缺失的 module」:行为真实存在,却从未被集中。

## Problem & Evidence(2026-07-07 grep 复核)

- `if ctx.json` 恰好 **85 处**、`print_enveloped(` 恰好 **85 处**,分布:library.rs 17、item/read.rs 12、collection.rs 12、workspace.rs 11、item/write.rs 9、note/config 各 5、tag/annotation 各 4、scite 3、sync/graph/doctor 各 1
- envelope meta(count/total/profile/api_version)组装随分支涂抹在各调用点(format.rs:26-59 只集中了一半)
- 契约违约:`src/zot-cli/src/commands/workspace.rs:185-206` 的 bibtex/markdown 分支在 `--json` 下直接 `println!`,未走 envelope——是否有意需在设计时确认;若有意,须在 spec 记录豁免
- 相关 spec:`.trellis/spec/zot-cli/backend/logging-guidelines.md`(JSON 模式规则)、`quality-guidelines.md`(envelope 契约)

## Requirements

- handler 返回结构化输出(数据 + meta + human 渲染所需信息),成功路径不再自行打印
- json/human 分支只在 dispatch 层(commands/mod.rs 一带)出现一次
- envelope meta 组装集中一处
- 错误路径保持现状:`ZotError` 下沉到 main.rs 统一打印的既有行为不变
- JSON 成功输出字节级不变(既有 JSON 契约测试为准);workspace export 例外按设计决定处理

## Acceptance Criteria

- [ ] grep `if ctx.json` 在 commands/ 下 ≤ 2 处(仅 dispatch 层)
- [ ] 既有 CLI 解析与 JSON 契约测试全绿,无输出格式回归
- [ ] workspace export 在 `--json` 下走 envelope,或在 spec 记录豁免理由
- [ ] 至少 3 个 handler 的返回值获得直接单元测试(不经 stdout 断言)
- [ ] `cargo clippy` / `cargo test` 全绿

## Notes

- 复杂任务:`task.py start` 前需 design.md(CommandOutput 形状、13 文件渐进迁移策略)+ implement.md(迁移清单 + 验证命令)。
- 顺序:建议先完成 07-07-envelope-err(本模块会使用其 `err()` 构造器);本任务是 07-07-pure-plan 与 07-07-library-seam 的形状前提。
- 父任务:07-07-arch-deepening(评审候选 A,Strong)。
