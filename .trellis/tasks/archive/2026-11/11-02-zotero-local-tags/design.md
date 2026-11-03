# Design — 本地标签

## CLI proposal

沿用：

- item tag add KEY --tag NAME ... --write-backend local
- item tag remove KEY --tag NAME ... --write-backend local
- 确认追加 --confirm --expected-server-id ID --expected-version N。

ItemTagUpdateArgs 增加 LocalWriteArgs；Web 分支使用 validate_web_options 拒绝 local-only 确认参数。既有 item tag batch 不在本子任务扩展。

## Flow

通过 LocalApiClient 读实例和条目 → 普通条目校验 → 计算 tags → 生成 MutationReport → 复用现有授权/条件 PATCH/回读。
preview/no-op 不读取授权内容、不写库；confirm 在授权前验证实例/条目版本。

## Tags semantics

- 解析为 tag 名称及原始对象，保留所有未删除条目的属性。拒绝形状不合法的服务端 tags，不以空数组代替。
- 新增精确同名已存在时 no-op，包含已有自动标签；新值创建手工标签。
- 移除匹配精确名称的所有关联（同名不同 type 也一并移除），预览完整展示；只作用于该 item。
- 输入重复名称去重；纯空白拒绝，其他名称保持输入，不做大小写/Unicode/同义词标准化。
- 不对既有不同标签去重或清洗，不将 type=1 改为手工标签。
- tags PATCH 是完整目标数组，body 只含 tags 与本地 version。
- 回读比较标签集合并保留重复数量语义；type 缺省与 0 视为同一手工类型，1 保持区分；不要求 API 数组顺序一致。未知属性不因比较而丢弃。

## Affected source / tests

- src/zot-cli/src/cli/args.rs
- src/zot-cli/src/commands/item/tag.rs
- src/zot-cli/src/commands/local_write.rs
- src/zot-cli/src/commands/local_write_tests.rs
- src/zot-desktop/src/local_api_tests.rs（只补协议缺口）
- src/zot-cli/tests/json_error_contract.rs
- docs/cli/{item,local-write}.md 及 docs/en 对应文件
- skills/zot/SKILL.md、必要的相关 reference/evals、.trellis/spec/zot-cli/backend/local-api.md

## Trade-off

保持显式标签操作，不开放任意 tags JSON 或 --field tags。这样能够保留用户既有组织结构，同时让 C 复用可测试的差异计算。现有 Web tags 路径可能有不同类型处理行为，本任务不顺便重写远端实现。

## Failure contract

沿用 local-api.md 的 preview/unchanged/applied/unknown/verification-failed 和明确拒绝错误；回读显示目标值但没有可靠写回执仍为 unknown。无自动后端 fallback。
