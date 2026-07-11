# 重复条目安全清理：跨类型合并加固与批量 dedupe

## Goal

让 `zot` 能对 Zotero 库中的重复条目做安全清理：每个重复组保留一条最优条目（keeper），其余条目的信息被吸收后移入回收站。跨 item type 的重复组（Zotero UI 因"必须同类型"拒绝合并的场景，如 conferencePaper + preprint + document）必须能处理，且已插入 Word/LibreOffice 文档的引文不断链。

用户价值：Zotero 自带功能与现存插件都无法清理跨类型重复（Zoplicate v3.0.2 起明确跳过跨类型组；ZoteroDuplicatesMerger 已停更且不兼容 Zotero 7）；手工逐组改类型再合并既繁琐又有字段丢失风险。

## Background（已确认事实）

细节与来源见 [research/2026-07-11-feasibility.md](./research/2026-07-11-feasibility.md)。要点：

- Zotero"同类型才能合并"仅是 UI 校验；数据模型允许跨类型合并。原生合并靠 keeper 上的 `dc:replaces` relation 保证文档引文不断链；裸删除会永久断链（无批量重连手段）。
- 写通道：本地 HTTP API 只读；唯一官方外部写通道是 Zotero Web API（zot 现行架构：本地 SQLite 只读检测 + Web API 写）。relations 字段可经 API 写入（官方示例即含 `dc:replaces`）；对条目类型非法的字段返回 400，不会被静默丢弃。
- 仓库已有：`library duplicates`（本地跨类型检测，src/zot-local/src/db.rs:1079）、`library duplicates-merge` / `item merge`（无同类型限制的合并引擎 `merge_item_set`，src/zot-cli/src/commands/item/merge.rs:28：补空字段 + 并 tags/collections + 子项 reparent + 源条目进回收站）、`item trash/restore`。
- 已确认缺陷：G1 合并不写 `dc:replaces`（引文断链）；G2 字段填充会把源类型专有字段写进 keeper 导致 PATCH 400（merge.rs:110 `is_none_or(is_empty)`）；G3 检测不排除回收站条目（db.rs:179 全量分支无 `deletedItems` 过滤）；G4 无批量入口与 keeper 自动选择。

## Requirements

### R1 引文保护（G1）

merge 执行（`--confirm`）时，keeper 的 `relations` 增加 `dc:replaces` → 每个被并条目的 URI（按 library scope 生成 `http://zotero.org/users|groups/{id}/items/{KEY}`）；源条目已有 relations 与 keeper 做并集迁移（替代现状"源 relations 对象整体填充进空 relations"）。此行为对 `item merge`、`library duplicates-merge`、`library dedupe` 三个入口一致生效。

### R2 跨类型字段安全（G2）

字段填充只写入对 keeper 类型合法的字段：只填「keeper flat JSON 中键存在且值为空」的字段，不再填「键缺失」的字段。dry-run preview 与 apply 结果需列出因类型不兼容被跳过的字段（字段名 + 来源条目），保证信息丢弃可见。

### R3 检测修正（G3）

`find_duplicates` 不得把回收站条目纳入重复组；`library search` / `library list` 等其他读路径行为保持不变（范围控制）。

### R4 批量清理入口 `library dedupe` + keeper 自动选择（G4）

新增 `zot library dedupe` 子命令（决策 2026-07-11）：

- 一条命令完成 检测 → 每组自动选 keeper → 输出整库/整 collection 清理计划；dry-run 默认，`--confirm` 后逐组执行，单组失败不中断、失败进入结果汇总。
- 复用 `library duplicates` 的检测与 `merge_item_set` 合并引擎；`duplicates` / `duplicates-merge` 保持现有语义不变（单组手工兜底）。
- 支持 `--method`、`--collection`、`--limit`，与检测命令对齐。
- keeper 默认策略（决策 2026-07-11）——"发表版优先"：类型优先级 journalArticle = conferencePaper（并列）> book/bookSection > thesis > report > preprint > document > 其他；同级 tie-break 依次比非空元数据字段数 → 本地附件数 → dateAdded 更早。计划的 `reason` 字段输出每组判定依据。
- 置信度标记（决策 2026-07-11）：title 匹配不加硬年份门槛；组内年份差 > 1，或组内存在两条及以上非空且互不相同的 DOI 时，该组标 `confidence: "low"` 并附说明（如 `year spread 2021↔2023` / `differing DOIs`），其余组为 normal。检测召回不变，风险靠 dry-run 审核提示。（注：不照抄 Zotero 官方"DOI 不同即否决"规则——arXiv DOI 与正式版 DOI 必然不同，否决会漏掉 preprint↔正式版这一首要场景。）

### R5 文档与 SKILL 对齐

skills/zot/SKILL.md（写操作风险分层、路由表、语义差异条目）、docs/en 与 docs 中文 CLI 页同步新命令与新行为；`library dedupe --confirm` 纳入高风险批量写清单。

## Acceptance Criteria

- [ ] AC1（R1）：fake 测试断言合并后 keeper PATCH payload 的 `relations["dc:replaces"]` 含全部源条目 URI；user 与 group scope 前缀各一例；源 relations 并集迁移有测试。
- [ ] AC2（R2）：keeper=conferencePaper、源=preprint（含 `repository` 等类型专有字段）的测试：合并计划不含对 keeper 非法的字段，preview 列出被跳过字段（字段名+来源）；现有同类型合并测试行为不变。
- [ ] AC3（R3）：回归测试——fixture 中把重复组某条目标记入 `deletedItems` 后，该条目不再出现在任何重复组；`library search` 现有测试不受影响。
- [ ] AC4（R4）：`library dedupe` dry-run 输出 JSON 计划（组、confidence、keeper、reason、absorb 列表、跳过字段）；`--confirm` 对多组场景逐组提交，注入单组失败后其余组仍完成且失败进入汇总（fake 测试）。keeper 策略测试覆盖：类型优先级、同级字段数 tie-break、附件数 tie-break、dateAdded tie-break 各至少一例；年份差 >1 与组内 DOI 互异两种 `confidence: "low"` 触发各有测试。
- [ ] AC5（R5）：SKILL.md 与 docs（EN/中文）包含 `library dedupe` 及新 preview 字段说明。
- [ ] AC6：`just ci` 通过（fmt / check / clippy -D warnings / test 全绿）。

## Out of Scope

- 永久删除（`emptyTrash` / DELETE）：清理只进回收站。
- 本地 SQLite 写入、插件桥（debug-bridge / 内嵌 MCP 插件）等非 Web API 写通道。
- Zoplicate 式"导入时实时去重"。
- ISBN 检测维度（库以论文为主）；`library search`/`list` 的回收站过滤（另行任务）。
- MCP server 面（`zot mcp serve` 未实现，不在本任务扩展）。
