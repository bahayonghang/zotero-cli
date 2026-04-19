# 安全边界

## 这些动作默认视为有副作用

- `item create`
- `item add-doi`
- `item add-url`
- `item add-file`
- `item update`
- `item trash`
- `item restore`
- `item attach`
- `item note add`
- `item note update`
- `item note delete`
- `item tag add`
- `item tag remove`
- `item tag batch`
- `item annotation create`
- `item annotation create-area`
- `collection create`
- `collection rename`
- `collection delete`
- `collection add-item`
- `collection remove-item`
- `library saved-search create`
- `library saved-search delete`
- `library duplicates-merge --confirm`
- `sync update-status --apply`
- `config init`
- `config set`
- `config profiles use`

## 执行规则

1. 如果用户明确要求执行，可以做
2. 如果用户只是想“分析”“看看”，不要偷偷写库
3. 这些动作要确认意图已经明确：
   - `item trash`
   - `item note delete`
   - `collection delete`
   - `library saved-search delete`
   - `library duplicates-merge --confirm`
   - `sync update-status --apply`

## 读写边界

- 本地读：`zotero.sqlite`、附件 storage、本地索引 sidecar
- 远端写：Zotero Web API

**永远不要直接改 `zotero.sqlite`。**

## 写权限缺失时怎么办

如果 `doctor` 显示凭据未配置：

- 停在只读分析
- 明确告诉用户缺少什么
- 不要假装操作已经成功

如果任务是配置排障：

- 先看 `config show`
- 需要写配置时再执行 `config init` / `config set`
- profile 切换也算有副作用

## annotation 与 attach_mode 的额外说明

- annotation 创建需要本地可读 PDF 和写权限同时可用
- `attach-mode auto` 找不到开放获取 PDF，不等于整个命令失败
