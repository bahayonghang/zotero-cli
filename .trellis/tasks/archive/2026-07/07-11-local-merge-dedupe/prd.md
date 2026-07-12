# 本机 merge 与 dedupe 闭环

## Goal

在 bridge foundation 上，将 `item merge`、`library duplicates-merge` 和
`library dedupe --confirm` 接入 Zotero 桌面客户端原生 merge，实现无 Web API key、
跨 item type、引文安全的本机 preview-confirm-apply 闭环，同时保留显式 web 后端。

## Dependencies

- **阻塞依赖**：`07-11-zotero-bridge-foundation` 已完成并通过 XPI、protocol、client、
  pairing/config/doctor 验收。
- **复用依赖**：归档任务 `07-11-dedupe-cleanup` 与提交 `bb5df55` 的 duplicate planner、
  keeper、confidence、字段兼容、`dc:replaces` 和 JSON model。
- **下游依赖**：本任务命令和 envelope 稳定后，`07-11-local-write-skill-docs` 才能开始。

## Requirements

- **R1 writer abstraction**：三个入口通过同一 selected merge writer；global
  `--write-backend desktop|web` 覆盖 profile，失败不 fallback。
- **R2 desktop preview**：插件按当前 Zotero items 计算 metadata fill、跳过字段、
  结构迁移摘要、版本指纹和短期 plan token；preview 不写库。
- **R3 desktop apply**：apply 重载 items 并比对 plan；在调用原生 `mergeItems()` 前
  只把 keeper 合法且为空的 metadata 设置在内存对象上，由 native merge 的事务统一保存。
- **R4 native ownership**：attachments、notes、relations、collections、tags、
  `dc:replaces`、duplicate attachments 和 source trash 由 Zotero 9 原生 merge 处理；
  插件不得复制这套算法。
- **R5 metadata compatibility**：保留 keeper item type；按 field/base-field 映射填空，
  不覆盖非空值，不写 keeper type 不支持的字段。Phase 1 不合成或替换 creators。
- **R6 candidate validation**：keeper/source 必须是同一 editable library 的未删除顶层
  bibliographic items；拒绝 child、attachment、note、annotation、重复 key、跨库和只读 group。
- **R7 backend-visible output**：`MergePreview`、`MergeApplyResult`、`DedupePlan`、
  `DedupeApplyReport` 增加 `write_backend`，human output 同步显示。
- **R8 low-confidence gate**：`library dedupe --confirm` 只执行 normal；low 组加入
  `skipped_low_confidence`。`--include-low-confidence` 才加入批量 apply。明确 keeper/source
  的 `item merge` / `duplicates-merge` 不重新推断 confidence，但仍需 `--confirm`。
- **R9 group isolation**：desktop 单组全事务；batch 逐组串行，失败进入 `failed` 后继续。
- **R10 idempotency**：相同 operation retry 返回缓存结果或 `already_applied`；响应丢失
  不得再次迁移或产生第二次 trash。
- **R11 web compatibility**：web writer 保留现有 request 顺序和结果；新抽象不能改变
  explicit web 的凭据门或自动调用 desktop。

## Acceptance Criteria

- [ ] **AC1** 无 API key 时，desktop `item merge` preview 不写库，confirm 后跨类型
  合并并返回 `write_backend: desktop`。
- [ ] **AC2** keeper 非空字段不覆盖；合法空字段填充；不兼容字段进入 skipped；
  creators 不被合成或替换。
- [ ] **AC3** PDF、带 annotation PDF、网页附件、普通附件、notes、tags、collections、
  relations 和 trash 与 Zotero 9 原生 merge 一致。
- [ ] **AC4** 跨库、child item、只读 group、deleted item、版本/child fingerprint 漂移
  在写入前拒绝，库状态不变。
- [ ] **AC5** apply 注入异常时原生事务完整回滚；重复 operation retry 不重复执行。
- [ ] **AC6** dedupe dry-run 保持本地且不要求 writer credential；输出计划 backend。
- [ ] **AC7** dedupe confirm 默认 normal-only；low 组出现在
  `skipped_low_confidence`，include flag 后才调用 writer。
- [ ] **AC8** 多组中一组失败时其他 normal 组继续，applied/failed/skipped counts 正确。
- [ ] **AC9** explicit web 的现有 merge tests、凭据错误和 JSON shape 保持兼容，
  desktop failure fake 断言没有 web request。
- [ ] **AC10** fake protocol、CLI tests、`just ci` 和隔离 Zotero fixture smoke 通过。

## Out of Scope

- 修改 duplicate detection、keeper 评分和 confidence 算法。
- note/tag/collection/import/attachment/saved-search/status-sync desktop writer。
- 永久删除、清空回收站、自动 fallback、任意 JavaScript。
- Phase 1 自动合并或重排 creators。
