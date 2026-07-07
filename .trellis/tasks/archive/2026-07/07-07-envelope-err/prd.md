# CliEnvelope 补 err() 构造器,消除 ErrorPayload 双胞胎

## Goal

`ErrorPayload`(error.rs:57)与 `EnvelopeError`(envelope.rs:33)结构完全相同(`{code, message, hint}`),二者之间的转换靠 format.rs 手抄字段完成。把「错误 → envelope」映射收进 zot-core,消除双胞胎。deletion test 直接通过:两个类型留一个即可。

## Problem & Evidence(2026-07-07 核实)

- `zot-core/src/error.rs:57-63` `ErrorPayload{code,message,hint}` ≡ `zot-core/src/envelope.rs:33-39` `EnvelopeError{code,message,hint}`
- `CliEnvelope` 只有 `ok` / `ok_with_meta`(envelope.rs:45-59),**没有 `err()`**
- 唯一消费点 `zot-cli/src/format.rs:41-59`:`print_error` 连续三次调用 `err.payload()`(:46,:47,:48)逐字段拷进 EnvelopeError
- spec 冻结项:`ZotError::payload()` → `ErrorPayload{code,message,hint}` 语义(zot-core error-handling.md:22-34)——在 JSON 输出字节不变的前提下做内部收敛

## Requirements

- zot-core 提供 `CliEnvelope::err(&ZotError)`(或等价单一入口);format.rs 不再手抄字段
- JSON 错误输出字节级不变(既有契约测试为准)
- ErrorPayload / EnvelopeError 收敛为一个类型(删一个或 alias),pub API 破坏最小化

## Acceptance Criteria

- [ ] format.rs 不再出现逐字段拷贝(每条路径 `payload()` 调用 ≤ 1 次)
- [ ] JSON 错误输出与现状逐字节一致(测试断言)
- [ ] `cargo clippy` / `cargo test` 全绿

## Notes

- 轻量:PRD-only 足够。
- **建议全树最先做**:07-07-cmd-output 的输出模块会直接使用 `err()` 构造器。
- 父任务:07-07-arch-deepening(评审小信号 1)。
