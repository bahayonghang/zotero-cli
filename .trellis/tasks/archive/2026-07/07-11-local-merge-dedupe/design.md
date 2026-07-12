# Design: 本机 merge 与 dedupe 闭环

## Writer Boundary

现有 `merge_item_set(&ZoteroRemote, ...)` 拆为 backend-neutral orchestration：

```text
MergeWriter
  preview(scope, keeper, sources) -> MergePreview + backend plan handle
  apply(scope, plan, operation_id) -> MergeApplyResult

WebMergeWriter     -> existing flat-item plan + ZoteroRemote calls
DesktopMergeWriter -> zot-desktop /merge/preview + /merge/apply
```

trait 位于 `zot-cli` command domain，因为它编排两个基础设施 client；协议 DTO 位于
`zot-desktop`。现有 `GroupMerger` 继续作为 dedupe failure-injection seam，但 production
实现改为持有 `&dyn MergeWriter`，避免 parallel backend hierarchy。

backend resolver 只在 apply/详细 preview 需要时构造 client。`library dedupe` 的本地
planner 不调用 writer；因此缺 Web key 或 Zotero 未运行不阻止默认 dry-run。

## CLI Surface

- global `--write-backend desktop|web` 由 foundation 提供。
- `LibraryDedupeArgs` 增加 `--include-low-confidence`；只有与 `--confirm` 同用时生效，
  不带 confirm 时可保留在 plan output 中标记“would include”，但不写库。
- relevant result models 增加 lower-case `write_backend`。
- `DedupeApplyReport` 增加：

```jsonc
{
  "write_backend": "desktop",
  "applied": [],
  "failed": [],
  "skipped_low_confidence": [],
  "total_groups": 10,
  "eligible_groups": 8,
  "applied_groups": 7,
  "failed_groups": 1,
  "skipped_low_confidence_groups": 2
}
```

## Desktop Protocol Extension

bridge v1 新增：

```text
POST /merge/preview
POST /merge/apply
```

preview request 只含 scope、keeper key、source keys 和通用 request meta。response 含：

- keeper/source current versions；
- canonical merge fingerprint；
- `metadata_fields_to_fill` 和 `skipped_incompatible_fields`；
- tags/collections/relations/children/duplicate-attachment summary；
- random opaque `plan_token`、`expires_at`；
- `confirm_required: true`。

plan token 是插件内存 map 的随机 256-bit key，默认两分钟过期，绑定 auth token hash、
scope、keys、fingerprint 和 preview。CLI 不解析 token，不写 config，也不输出到 human 或
JSON。普通 dry-run 只返回 `plan_id` 的脱敏标识；之后的 `--confirm` 调用重新 preview，
并在同一进程内立即携带 raw token apply，agent 不需要复制临时 secret。

apply request 含 plan token 和 caller-generated `operation_id`。plugin replay cache 绑定
operation ID；相同 payload 返回 cached result，不同 payload 拒绝。

## Candidate Resolution

插件把 public scope 映射为 local library：

- user -> `Zotero.Libraries.userLibraryID`
- group public ID -> `Zotero.Groups.getByGroupID(id).libraryID`

加载 keeper/sources 后验证：key 唯一、top-level、非 attachment/note/annotation、未 deleted、
同一 library、library editable。fingerprint 至少包含 item key/version/type/library/deleted、
merge-relevant field values，以及 source/keeper child key/version/linkMode/contentType 集合。

apply 重算 fingerprint；任何变化返回 `bridge-item-changed` 和重新 preview hint。

## Metadata Fill

沿用 web backend 的“只填空、不覆盖”合同，但使用 Zotero 运行时 field APIs：

1. 遍历 source 的非结构 metadata field；结构字段沿用现有排除集：key/version/type/
   dates/tags/collections/creators/parent/relations。
2. 对 source field 取得 base field；映射到 keeper type 的合法 field。
3. keeper 对应值为空时加入 fill；不合法时加入 skipped；非空时不动作。
4. source 按 CLI 传入顺序处理，第一条非空值获胜，行为与现有 web engine 一致。
5. apply 只对内存 keeper 调 `setField`，不单独 save，然后直接调用 Zotero 9 的
   `mergeItems(keeper, sources)`。native merge 保存 keeper 并完成一个事务，避免嵌套事务。

实现期必须以 Zotero 9 smoke 验证“预设的 in-memory fields 随 native merge 一起保存且
失败时不落盘”。若该假设不成立，停止实现并回到 design；不得退化为 merge 前独立 save。

creators 在第一阶段不合成，保留 keeper creators；tags/collections/relations/children
交给 native merge。插件返回的 preview summary 用于审计，不自行实施这些 structural moves。

## Dedupe Flow

```text
find_duplicates -> build_dedupe_plan (local, backend-independent)
  -> report selected/planned backend
  -> if !confirm: return plan
  -> partition normal / low
  -> eligible groups sequentially:
       writer.preview -> writer.apply
       success -> applied
       error   -> failed and continue
  -> low not included -> skipped_low_confidence
```

明确指定 keeper/source 的命令是用户选定单组，不再根据 local detector 重建 confidence。
它们仍要求 preview 后显式 confirm，并可用于人工处理 low group。

## Web Backend Compatibility

`WebMergeWriter` 封装当前 `get_item_flat -> build_merge_execution_plan -> update keeper ->
reparent -> set_deleted` 流程。添加 backend 字段但不更改动作顺序。web 没有事务，原有
幂等/部分失败说明保留；desktop 的事务保证不能被错误地写进 web 文档。

## Failure Semantics

- desktop group: transaction success or no persistent changes。
- batch: group-level partial success is expected and reported。
- timeout after apply: retry same operation ID；cached result 或 `already_applied`。
- already applied detection: all sources deleted 且 keeper `dc:replaces` 指向 sources 时返回
  success with `already_applied: true`；部分 source 状态不一致返回 transaction/state error。
- plugin version/protocol/auth/Zotero shutdown errors stay desktop；never call web。

## Test Strategy

- backend resolver/precedence/no-fallback tests。
- shared conformance cases run against fake WebMergeWriter and fake DesktopMergeWriter，验证
  preview/apply shape、candidate validation 和 output backend。
- plugin merge protocol mocks for field mapping, fingerprint, plan expiry/replay/idempotency。
- existing web merge tests remain; dedupe low partition tests added before writer fake。
- isolated Zotero 9 fixture covers cross-type metadata, each attachment class, notes, relations,
  read-only group rejection, rollback injection and UI trash verification。
