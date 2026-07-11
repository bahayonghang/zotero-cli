# Design：跨类型合并加固与批量 dedupe

对应 [prd.md](./prd.md) R1-R5；事实依据见 [research/2026-07-11-feasibility.md](./research/2026-07-11-feasibility.md)。

## 架构与模块边界

沿用现有分层，不引入新写通道：

```
zot-cli (命令编排、计划构建纯函数、JSON envelope)
  ├─ zot-local (只读检测：find_duplicates / get_attachments)
  ├─ zot-remote (Web API 写：PATCH keeper / reparent / set_deleted)
  └─ zot-core  (模型：MergePreview 扩展、Dedupe 计划/结果类型)
```

- **zot-local**：`SearchOptions` 新增 `exclude_trashed: bool`（默认 `false`，现有行为不变）；`search` 的取数分支追加 `i.itemID NOT IN (SELECT itemID FROM deletedItems)` 条件（仅当该开关为 true）；`find_duplicates`（db.rs:1079）以 `exclude_trashed: true` 取数。附件计数复用现有 `get_attachments`（db.rs:922）按组内条目循环，不新增 SQL。
- **zot-remote**：新增 `item_uri(&self, key) -> String`，按 `LibraryScope`（zotero.rs:613）生成 `http://zotero.org/users|groups/{library_id}/items/{KEY}`。写路径复用现有 `update_flat_item_value` / `set_deleted`，不新增端点。
- **zot-cli**：`build_merge_execution_plan`（merge.rs:69）保持纯函数（无 IO）；新增入参为「源条目 key→URI 映射」，由调用方（`merge_item_set`）用 `remote.item_uri` 预构造。dedupe 的 keeper 评分、confidence 计算、计划构建同样是纯函数，模块位置按 zot-cli spec 的目录规范放在 library 命令域内（实现期由 before-dev 确认，倾向 `commands/library_dedupe.rs` 或 `commands/library/` 子模块，与 `commands/item/merge.rs` 的先例对齐）。
- **zot-core**：`MergePreview`/`MergeApplyResult` 扩展字段；新增 `DedupePlan` / `DedupeGroupPlan` / `DedupeApplyReport` 模型。

## 数据流

dry-run（默认）：`find_duplicates(exclude_trashed)` → 每组本地评分选 keeper → `DedupePlan`（纯本地，不触网）。

`--confirm`：对计划逐组调用现有 `merge_item_set`（merge.rs:28，内部再走 远端 fetch flat → 构建执行计划 → PATCH keeper → reparent children → trash sources）；单组错误被捕获后继续下一组，汇总进 `DedupeApplyReport`。

## 契约

### MergePreview / MergeApplyResult 扩展（R1/R2，三个入口共享）

```jsonc
{
  // 现有字段不变，新增：
  "skipped_incompatible_fields": [ { "field": "repository", "source_key": "PREP01" } ],
  "relations_to_add": [ "http://zotero.org/users/123/items/PREP01" ]  // dc:replaces 值
}
```

JSON envelope 只增不改，向后兼容；docs 标注新增字段。

### DedupePlan（dry-run 输出 data）

```jsonc
{
  "groups": [ {
    "match_type": "title", "confidence": "low",           // normal | low
    "confidence_note": "year spread 2021↔2023 (>1)",       // 仅 low 时存在
    "keeper": { "key": "CONF01", "item_type": "conferencePaper", "title": "…" },
    "reason": "type: conferencePaper(1) > preprint(5); tie-break: fields 8>5, attachments 1>0",
    "absorb": [ { "key": "PREP01", "item_type": "preprint", "title": "…" } ]
  } ],
  "total_groups": 17, "confirm_required": true
}
```

### DedupeApplyReport（--confirm 输出 data）

```jsonc
{
  "applied": [ /* 每组 MergeApplyResult + keeper key */ ],
  "failed":  [ { "keeper": "K2", "sources": ["D3"], "error": "remote: 412 precondition failed" } ],
  "total_groups": 17, "applied_groups": 16, "failed_groups": 1
}
```

## 关键算法

### keeper 评分（确定性、可解释）

1. 类型优先级（rank 越小越优）：journalArticle=conferencePaper(1) > book=bookSection(2) > thesis(3) > report(4) > preprint(5) > document(6) > 其他(7)。
2. tie-break 依次：非空元数据字段数（基于本地 `Item` 压缩模型：title/abstract_note/date/url/doi/creators/tags/extra 计数）→ 本地附件数（`get_attachments`）→ `date_added` 更早 → key 字典序（最终兜底，保证确定性）。
3. `reason` 字符串按实际起作用的比较层生成。

### 字段填充过滤（R2）

现状 merge.rs:110 `target_value.is_none_or(is_empty)` 改为「键存在且值为空」才填；键缺失的非空源字段记入 `skipped_incompatible_fields`。

**假设 A**（此修法的前提）：Web API GET item 的 `data` 含该类型全部合法字段（未设值为 `""`）。实现期先对真实库 `zot --json item get <一条 conferencePaper>` 验证；若不成立，兜底方案：`GET /itemTypeFields?itemType=X`（免鉴权）拉合法字段集并进程内缓存。两方案收敛到同一个判定函数，切换不影响调用方。

### 置信度标记

`confidence: "low"` 的触发条件（满足其一）：① 组内年份差 > 1；② 组内存在 ≥2 个非空且互不相同的 DOI。`confidence_note` 说明具体触发原因。刻意**不采用** Zotero 官方"DOI 不同即否决重复"规则——arXiv DOI（`10.48550/…`）与正式版 DOI 必然不同，否决会漏掉 preprint↔正式版这一首要清理场景；降级为提示而非排除。

### relations 并集 + dc:replaces（R1）

keeper.relations 与各源 relations 按 predicate 并集（值统一数组化、去重）；再为每个源 key 追加 `dc:replaces: [item_uri(source)…]`。重跑幂等（去重保证）。字处理插件对 `dc:replaces` 的检查有官方背书（dstillman，forums/78483："We do check them for word processor citations"）。

## 权衡记录

- **不全局排除回收站**：`search`/`list_items` 语义变化会波及 semantic index、workspace import 等下游，本任务只在 find_duplicates 路径开启开关（PRD R3 范围控制）。
- **完整度评分用本地压缩模型而非远端全字段**：dry-run 必须离线、快；压缩模型足以区分 keeper；执行期才触网。
- **--confirm 无组间/组内回滚**：Web API 无事务；组内半途失败留下的状态安全（trash 可恢复、fill 只补空、reparent/trash/relations 均幂等或可重入），重跑 dedupe 收敛。
- **不改 duplicates/duplicates-merge CLI 语义**：单组手工路径是批量的兜底与逃生门。
- **被跳过的同签名重复附件随源条目进回收站**：子项 reparent 先于 trash；Web API 对父条目写 `deleted:1` 不级联子件（dataserver #42），这些附件保持为回收站条目的子件、清空回收站时一并擦除——属预期，不做额外处理。

## Rollout / Rollback

- 无迁移、无本地 schema 变化；合入即用。
- 代码回滚：各阶段独立提交，逐段 revert。
- 数据回滚：清理均为软删除（Zotero 回收站可恢复）；`dc:replaces` 如需撤销可经 API 移除（docs 注明 Zotero UI 不直接展示 relations）。
