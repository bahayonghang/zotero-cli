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

1. 新环境或任何写入先跑 `zot --json doctor`，分别读取四类 capability
2. 如果用户只是想“分析”“看看”，不要偷偷写库
3. 只有 BibTeX/RIS 新增导入可走 connector；merge/dedupe 与其他 mutation 走 Web API
4. 这些动作要确认意图已经明确：
   - `item trash`
   - `item note delete`
   - `item merge --confirm`
   - `collection delete`
   - `library saved-search delete`
   - `library duplicates-merge --confirm`
   - `library dedupe --confirm`
   - `sync update-status --apply`
5. merge/dedupe 先 preview，复述 keeper、sources、confidence 和跳过项，再等确认
6. `library dedupe --confirm` 默认跳过 low-confidence。普通 confirm 不代表风险授权；只有单独展示这些组并取得明确授权后才可追加 `--include-low-confidence`

## 读写边界

- 本地读：`zotero.sqlite`、Zotero Local HTTP、附件 storage、本地索引 sidecar
- connector 写：只新增导入 BibTeX/RIS，且确认前重新检查 Zotero UI 当前目标可写
- Web 写：merge/dedupe、note、tag、collection、Web import、annotation、saved-search、status-sync

**永远不要直接改 `zotero.sqlite`，也不要把 Local HTTP 描述成写通道。**

## 写权限缺失时怎么办

如果 `doctor` 显示所需 capability 不可用：

- 停在只读分析
- connector：启动 Zotero，并在 UI 中选择可写的 library / collection
- Web：明确缺少 `ZOT_LIBRARY_ID` / `ZOT_API_KEY`
- 不要假装操作已经成功

如果任务是配置排障：

- 先看 `config show`
- 需要写配置时再执行 `config init` / `config set`
- profile 切换也算有副作用

## connector 目标边界

- connector 无需插件、配对码或 token
- dry-run 与 confirm 都先读取当前目标；confirm 在真正 import 前再次检查 `editable` / `libraryEditable`
- 只读目标必须在发送 import 请求前失败关闭
- 不在日志、prompt、issue、fixture 或文档中记录 API key

## annotation 与 attach_mode 的额外说明

- annotation 创建需要本地可读 PDF 和写权限同时可用
- `attach-mode auto` 找不到开放获取 PDF，不等于整个命令失败
