# Implement：执行计划

前置：`task.py start` 后进入 Phase 2；每阶段结束跑对应验证命令；阶段间是独立回滚点（可独立提交）。

## 阶段 1 — R3 检测修正（zot-local）

- [ ] `SearchOptions` 加 `exclude_trashed: bool`（默认 false；`Default` 派生保持现状）
- [ ] `search` 全量分支与 LIKE 分支在开关开启时追加 `deletedItems` 排除（对齐 db.rs:313 现有写法）
- [ ] `find_duplicates` 取数改走 `exclude_trashed: true`
- [ ] 测试：复用测试 fixture 的 `deletedItems` 表（db.rs:2516 附近）——组内一条入回收站后不再出现在任何组；`library search` 现有测试不变
- 验证：`cargo test -p zot-local`

## 阶段 2 — R1+R2 合并引擎加固（zot-core / zot-remote / zot-cli）

- [ ] zot-core：`MergePreview`/`MergeApplyResult` 增 `skipped_incompatible_fields`、`relations_to_add`
- [ ] zot-remote：`item_uri(key)`（user/group 两种 scope）+ 单测
- [ ] 验证设计假设 A（GET item 返回全字段模板）：真实库 `zot --json item get` 一条 conferencePaper 确认空字段以 `""` 在场；不成立则实现 `/itemTypeFields` 兜底（见 design.md）
- [ ] merge.rs：fill 条件改「键存在且空」；收集 skipped 字段；relations 并集 + `dc:replaces` 注入（URI 由 `merge_item_set` 传入，纯函数不触网）
- [ ] `item merge` / `library duplicates-merge` 两入口透传 URI；preview/applied 输出新字段
- [ ] 测试：跨类型（conferencePaper keeper + preprint 源含 `repository`）不产生非法字段且 skipped 可见；user/group URI 各一例；relations 并集去重；现有同类型测试仍绿
- 验证：`cargo test -p zot-cli -p zot-core -p zot-remote`

## 阶段 3 — R4 `library dedupe`（zot-core / zot-cli）

- [ ] zot-core：`DedupePlan` / `DedupeGroupPlan` / `DedupeApplyReport` 模型
- [ ] keeper 评分 + confidence 纯函数模块（类型 rank、字段数、附件数、dateAdded、key 兜底；year spread>1 → low）
- [ ] `cli/args.rs`：`LibraryDedupeArgs { method, collection, limit, confirm }`；`cli.rs` 接线 `LibraryCommand::Dedupe`
- [ ] `commands/library`：编排 检测→计划→（--confirm）逐组 `merge_item_set`、单组失败 catch 继续、汇总
- [ ] 测试：沿 collection.rs 的 fake seam 模式——计划 JSON 形状、keeper 策略 4 个 tie-break 层各一例、low-confidence 两种触发（年份差 >1、组内 DOI 互异）各一例、多组执行中注入单组失败其余完成
- 验证：`cargo test -p zot-cli`

## 阶段 4 — R5 文档与 SKILL

- [ ] `docs/en/cli/library.md` + `docs/cli/library.md`：`dedupe` 章节、duplicates-merge 新 preview 字段
- [ ] `skills/zot/SKILL.md`：路由表（duplicates 行附近，SKILL.md:147）、层 B/层 C 风险清单（SKILL.md:294/313/319）加 `library dedupe --confirm`、语义差异条目（dedupe vs duplicates-merge）
- [ ] README「Direct runtime reference」示例可选补一行
- 验证：`just ci`；docs 本地目测

## 收尾

- [ ] 最后一轮全范围质量检查（trellis-check：spec 对照、clippy -D warnings、测试、跨层数据流）
- [ ] 真实库小范围演练：`zot --json library dedupe --collection <小集合>` dry-run 人工核对计划，再 `--confirm` 一组验证端到端（含 Zotero 客户端 sync 后 UI 观感）

## 风险文件

- `src/zot-cli/src/commands/item/merge.rs` — 三个入口共享引擎，保持现有 preview 消费者兼容（只增字段）
- `src/zot-local/src/db.rs` `search` — 开关必须默认 false，防止波及 search/list/semantic 下游
