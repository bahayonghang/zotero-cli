# 可行性调研：跨类型重复条目安全清理（2026-07-11）

三路调研（Zotero 源码 / 写入通道 / 插件生态）+ 官方文档核实的合并结论。供本任务 design/implement 阶段引用。

## 1. Zotero 行为事实（源码级，zotero/zotero @ 5fe4c901，7.0 线）

- **重复检测**（`chrome/content/zotero/xpcom/duplicates.js`）：ISBN（仅书籍）、DOI（全类型）、标题归一化（去变音符/标点→空格/lowercase）+ 作者匹配，年份容差 ±1（`Math.abs(yearA-yearB) > 1 → 0 分`）。检测**不按 itemType 分桶**——跨类型条目会进同一重复组。
- **合并**：后端 `chrome/content/zotero/mergeItems.mjs` 只要求同一 library；**"Merged items must all be of the same item type" 是 UI 层校验**（`elements/duplicatesMergePane.js setItems()`），非数据模型限制。
- **引文保护机制**：合并时 `moveRelations()` 在保留条目上写 `dc:replaces` relation（`relations.js: replacedItemPredicate = 'dc:replaces'`），Word/LibreOffice 插件靠它把旧引文重定向到 keeper。
- **setType**：可经共享 base field 迁移的字段保留，否则清空（数据丢失）→ "改类型再合并"的 workaround 有损。

## 2. 写入通道（2026-07 现状）

| 通道 | 能力 | 结论 |
| --- | --- | --- |
| 本地 HTTP API `localhost:23119/api/` | 只读（`server_localAPI.js` L37 "Write access is not yet supported"，全端点 GET-only；写入端点 PR zotero/zotero#5015 截至 2026-06-23 未合并） | 只能用于读/检测 |
| Connector 端点 `/connector/saveItems` | 仅创建新条目 | 不可改/删已有条目 |
| **Zotero Web API v3**（api.zotero.org） | 完整写：`deleted:1` 进回收站、PATCH `parentItem` 挪子项、**relations 可写（官方文档写入示例即含 `dc:replaces`）**、批量 50 对象/请求、`If-Unmodified-Since-Version`/412 并发控制、**无效字段返回 400 "Invalid type/field"（不静默丢弃）** | ✅ 唯一官方外部写通道，zot 现行架构 |
| 直接写 zotero.sqlite | 官方不支持，破坏同步 | 禁止（README 边界） |
| 插件桥（BBT debug-bridge / 内嵌 MCP 插件 / zotero-local-write-api） | HTTP→任意 JS，可本地写 | 可行但要求用户装插件、改变 zot 安全边界 → 本任务不采用 |

## 3. 生态空白（不重复造轮子的依据）

- **Zoplicate**（927★，活跃，最流行去重插件）：v3.0.2 起明确"不同 itemType 不视为重复"→ 跨类型组根本不进检测。
- **ZoteroDuplicatesMerger**：有 force-type 跨类型合并，但 2022 年停更、不兼容 Zotero 7。
- pyzotero：Web API 无 merge 端点，社区脚本只能 delete-keep-one（断引文）。
- 官方立场：只提供 merge，不提供"删多余"（forum discussion/75135）；Run JavaScript 循环 `Zotero_Duplicates_Pane.merge()` 的 hack 在跨类型组上会失败（走的 UI 逻辑）。

## 4. 引文断链证据

- forum discussion/85939、88675：被引条目删除（非 merge）后，文档引文只剩嵌入数据、不可更新，无批量重连（只能逐条删了重插）。
- → 清理必须走"merge 语义 + dc:replaces"，不能裸 trash。

## 5. 本仓库现状与缺口

已有：

- `library duplicates`（src/zot-local/src/db.rs:1079）：本地只读检测，DOI 精确（lowercase）+ 标题 normalized Levenshtein ≥ 0.92，跨类型分组，`--method both|doi|title`、`--collection`、`--limit`（上限 list_items 10_000）。
- `library duplicates-merge` / `item merge`（src/zot-cli/src/commands/item/merge.rs:28）：dry-run 默认；执行 = 补 keeper 空字段 → 并集 tags/collections → 子项 reparent 到 keeper（按 contentType+filename+md5+url 签名跳过重复附件）→ 源条目 `set_deleted(true)` 进回收站。无同 type 限制（仅拒绝 attachment/note/annotation 参与）。
- `item trash/restore`；`db.get_attachments(key)`（db.rs:922）可查每条目附件。
- Web API 客户端（src/zot-remote/src/zotero.rs）：`get_item_flat`/`list_children_flat`/`update_flat_item_value`（PATCH 全量 flat 对象 + If-Unmodified-Since-Version）/`set_deleted`；`endpoint()` 已区分 user/group scope（zotero.rs:613）。

缺口：

- **G1 引文断链**：merge 不写 `dc:replaces`（merge.rs 无 relations 处理；且 fill 逻辑会把源 relations 对象整体填进空 relations 而非并集）。
- **G2 跨类型 400**：fill 用 `target_value.is_none_or(is_empty)`（merge.rs:110）——keeper flat JSON 缺失的键（= 对 keeper 类型非法的字段）也会被填入 → PATCH 400。修法：只填「键存在且值为空」的字段（依赖 API 返回全字段模板的行为，实现期验证；兜底 `/itemTypeFields`）。
- **G3 检测含回收站条目**：`search` 全量分支（db.rs:179）无 `NOT IN (SELECT itemID FROM deletedItems)` 过滤 → 清理后复检误报。
- **G4 无批量入口 + keeper 自动选择**：每组需人工指定 keeper；无整库 plan/apply 工作流。
- （可选）检测精度：title 匹配可加 year±1 门槛对齐官方算法，降低同名异文误报。

## 6. 工程约束（spec/仓库契约）

- zot-local 对 zotero.sqlite 只读；zot-remote 不碰 CLI/本地库；错误映射按各包 error-handling 指南。
- CLI 写操作惯例：merge 家族用 `--confirm` 做 dry-run 门（sync 用 `--apply`）。
- 新 CLI 面积需同步 skills/zot/SKILL.md（风险分层 层B/层C 清单、路由表）+ docs/en + docs 中文。
- 测试惯例：collection/note 命令经 in-file Fake + 泛型 seam 测试（2026-07-10 commit c0adb62）；merge 目前直接依赖具体 `ZoteroRemote`，新逻辑应沿 seam 模式做 fake 测试。

## 7. 终版报告补充核证（2026-07-11 晚）

- **官方判重细则**（duplicates.js）：DOI 归一化 = trim+大写+须以 `10.` 开头；作者匹配 = 姓+名首字母至少一个交集（双方都有作者但零交集 → 否决；仅一方有作者 → 否决）；**双方 DOI 都非空且不同 → 否决重复**。⚠ 该 DOI 否决规则不可照抄进 zot：arXiv DOI（10.48550/…）与正式版 DOI 必然不同，照抄会排除 preprint↔正式版组（本任务首要场景）——改为作为 low-confidence 信号。
- **dc:replaces 官方背书**：dstillman（forums/78483）："Merged items are actually dc:replaces, not owl:sameAs… We do check them for word processor citations, so that should work."；官方文档 duplicate_detection 明言 "You should always resolve duplicate items by merging them, rather than deleting"。
- **trash 不级联子件**（dataserver #42）：对父条目写 `deleted:1` 不保证其子附件同置 trash。对 zot merge 流无害（子项先 reparent 到 keeper；被"同签名跳过"的重复附件留在源条目下随其进回收站，属预期），design 已注明。
- **群组库无 linked_file**：群组库根本不允许 linked_file 附件，reparent 不会遇到该类型；个人库无限制。
- **批量优化余地**：POST /items 可一次 50 个对象带 `deleted:1`（库版本头）；组内条目通常 2-4 条，现有逐条 PATCH 足够，暂不优化。
- **改 itemType via API**：不重新提交的旧字段静默 base-map/清除；显式提交对新类型非法的字段才 400 —— 本任务不改类型，仅作旁证记录。

## 主要来源

- https://github.com/zotero/zotero （duplicates.js / mergeItems.mjs / duplicatesMergePane.js / server_localAPI.js / relations.js）
- https://github.com/zotero/zotero/pull/5015 （本地 API 写入端点，未合并）
- https://www.zotero.org/support/dev/web_api/v3/write_requests （批量 50、412、400 invalid field、relations 含 dc:replaces 示例、parentItem 可改）
- https://forums.zotero.org/discussion/85939 、/88675 、/75135 、/127646
- https://github.com/ChenglongMa/zoplicate 、https://github.com/frangoud/ZoteroDuplicatesMerger
- https://github.com/retorquere/zotero-better-bibtex （test/fixtures/debug-bridge，未采用仅记录）
