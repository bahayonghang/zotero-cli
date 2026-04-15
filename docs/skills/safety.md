# 安全边界

## 这些动作默认视为有副作用

- `item create`
- `item update`
- `item trash`
- `item restore`
- `item attach`
- `item note add/update`
- `item tag add/remove`
- `collection create/rename/delete/add-item/remove-item`
- `sync update-status --apply`

## 执行规则

1. 用户已经明确要求执行：可以做
2. 用户只是“分析一下”“看看”：不要偷偷写库
3. 破坏性动作要确认意图明确：
   - `item trash`
   - `collection delete`
   - `sync update-status --apply`

## 读写边界

- 本地读：`zotero.sqlite` 与附件 storage
- 远端写：Zotero Web API

**永远不要直接改 `zotero.sqlite`。**

## 写权限缺失时怎么办

如果 `doctor` 显示凭据未配置：

- 停在只读分析
- 明确告诉用户缺少什么
- 不要假装操作已经成功
