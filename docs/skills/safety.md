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
- `item merge --confirm`
- `collection create`
- `collection rename`
- `collection delete`
- `collection add-item`
- `collection remove-item`
- `library saved-search create`
- `library saved-search delete`
- `library duplicates-merge --confirm`
- `library dedupe --confirm`
- `sync update-status --apply`
- `config init`
- `config set`
- `config profiles use`

## 执行规则

1. 新环境或任何写入先跑 `zot --json doctor`，分别读取四类 capability 和 `selected_write_backend`
2. 如果用户只是想“分析”“看看”，不要偷偷写库
3. 明确 desktop/Web 时只使用该 backend；未明确时使用 effective profile。失败保留在原 backend，不自动 fallback
4. 这些动作要确认意图已经明确：
   - `item trash`
   - `item note delete`
   - `item merge --confirm`
   - `collection delete`
   - `library saved-search delete`
   - `library duplicates-merge --confirm`
   - `library dedupe --confirm`
   - `sync update-status --apply`
5. merge/dedupe 先 preview，复述 keeper、sources、backend、confidence 和跳过项，再等确认
6. `library dedupe --confirm` 默认跳过 low-confidence。普通 confirm 不代表风险授权；只有单独展示这些组并取得明确授权后才可追加 `--include-low-confidence`

## 读写边界

- 本地读：`zotero.sqlite`、Zotero Local HTTP、附件 storage、本地索引 sidecar
- desktop 写：配对插件当前只支持 `item merge`、`library duplicates-merge`、`library dedupe`
- Web 写：note、tag、collection、import、annotation、saved-search、status-sync，以及显式选择 Web 的 merge/dedupe

**永远不要直接改 `zotero.sqlite`，也不要把 Local HTTP 描述成写通道。**

## 写权限缺失时怎么办

如果 `doctor` 显示所选 capability 不可用：

- 停在只读分析
- desktop：区分 Zotero 未运行、插件未安装、未配对、auth/protocol/profile mismatch，并给原 backend 的恢复动作
- Web：明确缺少 `ZOT_LIBRARY_ID` / `ZOT_API_KEY`
- 不要假装操作已经成功
- 不要自行切换 backend

如果任务是配置排障：

- 先看 `config show`
- 需要写配置时再执行 `config init` / `config set`
- profile 切换也算有副作用

## bridge secret 与安装边界

- `bridge setup` 只生成 XPI 并打开目录，不会自动安装或修改 Zotero profile
- pairing code 五分钟过期且单次使用，只由 Zotero UI 显示
- 不在日志、prompt、issue、fixture 或文档中记录真实 code、desktop token、API key 或 raw plan token
- `bridge revoke` 用于撤销当前授权；同一 profile 的插件升级会保留连接身份

## annotation 与 attach_mode 的额外说明

- annotation 创建需要本地可读 PDF 和写权限同时可用
- `attach-mode auto` 找不到开放获取 PDF，不等于整个命令失败
