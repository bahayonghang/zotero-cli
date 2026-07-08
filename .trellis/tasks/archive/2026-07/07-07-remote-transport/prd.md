# 补完 zot-remote transport seam

## Goal

让 `http.rs`(现 65 行,只共享连接池)长成 deep transport module:6 个 client 的错误映射、响应处理、base URL 收敛一处;HTTP 边界获得第二个 adapter(本地 fake)从而第一次可验证。907 行零测试的 zotero.rs(写关键路径)是最大受益者。

## Problem & Evidence(2026-07-07 grep 复核)

- `fn remote_err` 定义 **6 份**:better_bibtex.rs:100 / oa.rs:598 / scite.rs:256 / embedding.rs:107 / semantic_scholar.rs:203 / zotero.rs:886(其中 4 份逐字相同)
- send→status→json 的响应处理只在 zotero.rs 内部集中(ensure_empty:737 / ensure_json:752 / http_hint:895),其余 5 个 client 各自内联(oa:5 处 send / scite:4 处)
- base URL:oa / scite / better_bibtex / embedding 可覆写(env/config);`zotero.rs:13` 与 `semantic_scholar.rs:8` 为 `const API_BASE` 硬编码——seam 修了一半
- zot-remote 无 dev-dependencies、无 mock server;所有既有测试只打纯 helper
- spec 约束:`.trellis/spec/zot-remote/backend/quality-guidelines.md:24-32` network-test pattern =「测纯 helper,不打 live service」(fake server 是本地 adapter,不违背);`database-guidelines.md` = 无持久化

## Requirements

- transport 能力(错误映射、状态检查、JSON 解码、base_url 持有)集中一处;client 只描述各自 API 形状
- 全部 6 个 client 的 base URL 统一可覆写(供测试 adapter 使用)
- 新增针对 transport 与 zotero 写前置条件的测试(本地 fake server 或等价 adapter;不打 live service)
- HttpRuntime 的共享连接池语义、超时与 User-Agent 缺省不变

## Acceptance Criteria

- [ ] grep `fn remote_err` 定义 = 1 处
- [ ] 6 个 client 无内联 status→json 重复(抽查确认)
- [ ] zotero.rs 至少覆盖:版本前置条件、错误映射、一条写路径(经 fake adapter)
- [ ] 既有纯 helper 测试全绿;不引入持久化
- [ ] `cargo clippy` / `cargo test` 全绿

## Notes

- 复杂任务:`task.py start` 前需 design.md(transport interface 形状、fake adapter 选型:本地 server vs base_url 注入)+ implement.md。
- 与 07-07-pdf-http 同期评估(pdf.rs 的 blocking 下载是否统一到共享 transport)。
- 独立于 cmd-output / rag-engine,可并行。父任务:07-07-arch-deepening(评审候选 B,Strong)。
