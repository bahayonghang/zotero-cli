# 导入与安全写入

先满足入口里的确认、版本和禁止跨后端 fallback 规则，再使用这里的命令细节。

### 6. 导入文献到 Zotero

用户通常会说：

- “把这个 bib 导入 Zotero”
- “把这几条 RIS 存进我当前的 collection”

唯一写入路由是 `item import`：

- 文件输入：`item import --file <path>`；原始文本输入：`item import --text <text>`
- 只在自动识别不可靠时显式加 `--format bibtex|ris`
- 不带 `--confirm` 先 dry-run，读取并复述 Zotero UI 当前选中的目标 collection/library、目标可写性、记录数和格式
- 用户确认上述目标与记录数后，才用同一输入执行 `item import ... --confirm`
- confirm 前再次读取当前目标。目标或可写性变化时中止，重新 preview，不沿用旧结果

边界：

- connector 只会新增 BibTeX/RIS 条目，不支持 update、tag、note、collection mutation 或 merge/dedupe
- 目标由 Zotero UI 当前选择决定，不能在命令里指定 collection
- Zotero 未运行、connector 不可达或目标只读时停止；不要改走 Web API

### 7. 安全写入

用户通常会说：

- “给这篇文献加一条 note”
- “打上 priority 标签”
- “把所有 reading-list 标签的条目都加上 priority”
- “把这个条目挂到某个 collection”
- “先预览再合并这两篇文献”
- “合并重复条目”
- “把 preprint 的正式发表信息写回去”

先区分能力和后端：

| 路径 | 当前能力 | 凭据 / 运行条件 |
| --- | --- | --- |
| local SQLite / Local HTTP reads | 查询、提取和 dedupe planning | SQLite 永远不能写 |
| Zotero 10+ Local API | item update、collection create/add-item/remove-item/move-item、item tag add/remove | 显式 local 后端、预览/确认、Zotero 桌面授权；无需 Web key |
| connector 本机导入 | 仅 `item import` 新增 BibTeX/RIS 条目 | Zotero 正在运行、connector 可达、UI 当前目标可写 |
| Zotero Web API | 既有 mutation 默认后端，含 merge/dedupe 与 item tag batch；不含 move-item。item tag add/remove 未指定 local 时仍走这里 | `ZOT_LIBRARY_ID` + `ZOT_API_KEY` 或对应 profile 配置 |

写入决策顺序：

1. 先跑 `zot --json doctor`，读取 `capabilities.local_api_write`、`capabilities.connector_write`、`capabilities.web_write` 和 `write_credentials`。
2. BibTeX/RIS 新增导入只走 connector：先不带 `--confirm` 预览目标、可写性、记录数和格式，复述后取得确认，再执行 `item import ... --confirm`。
3. 本地字段/作者、分类创建/成员或单条标签操作优先评估 `local_api_write` 并按下一节显式选择 local；其他 mutation 走 Web API。Web 凭据缺失时停止对应 Web 操作；不要切换到 connector。
4. 对 merge/dedupe 先跑不带 `--confirm` 的本地 preview，复述 keeper、sources、confidence 和跳过项；执行 `--confirm` 前再检查 Web 凭据。

命令路由：

- connector 写：仅 `item import`。
- Web API 写：`item create/add-doi/add-url/add-file/update/trash/restore/attach`、`item note add/update/delete`、未指定 local 的 `item tag add/remove`、`item tag batch`、未指定 local 的 collection mutation、saved-search mutation、annotation creation、`sync update-status --apply` 和 merge/dedupe。
- Local API 写：`item update`、`collection create/add-item/remove-item/move-item`、`item tag add/remove`，都必须显式带 `--write-backend local`。
- `library dedupe --confirm` 默认只执行 normal-confidence 组；low-confidence 组保留在 `skipped_low_confidence`。普通 confirm 不等于 low-confidence 授权；只有用户看过 preview 后另行、明确接受这部分风险，才可追加 `--include-low-confidence`。
- 不要宣称 connector 能执行 tag、note、collection、attachment、annotation、saved-search、status-sync 或 merge/dedupe mutation。

### 7a. 本地字段、作者与分类成员（Zotero 10+）

自然语言“把条目从 A 移到 B”“补上这篇的 DOI/作者”可使用官方 Local API，不要求 Web 凭据。

1. 先 doctor：`local_api_write.supported` 仅证明版本/协议支持，`credential_present` 仅表示缓存授权，`verified=false` 不能解读成真实写入通过。Zotero 必须运行并启用本地通信。
2. 用 item key 与 collection key 确定同一 library 的目标；不要把 citation key 或 UI 当前选中分类当作 API 身份。必要时先检索解析，不能猜 key。
3. `item update K --write-backend local --field DOI=...` 或 `collection move-item K --from A --to B --write-backend local` 默认只预览。add-item/remove-item 保留 `<collection-key> <item-key>` 参数顺序，同样可选择 local。
4. 展示 before/after/skipped_fields；默认仅填空，覆盖非空字段需显式 `--overwrite`。`--creators-file` 为有序 JSON 数组，含 creatorType 与个人 firstName/lastName 或机构 name；已有作者列表需显式覆盖。
5. 用户认可具体修改后，保持原参数并增加 `--confirm --expected-server-id <预览值> --expected-version <预览值>`。Zotero 自身会在需要时显示授权框，不能替用户点击 Always Allow。
6. applied 且 verified 才表示确认成功；unknown 表示结果未知，verification-failed 表示写被接受但回读未验证，不能称为全部成功或盲重试。冲突须重新预览。

移动仅改变指定分类归属，保留其他分类、item key 和 children。不支持跨库移动、itemType 转换、批量修改、自动联网补全或本地 notes/merge。单条本地标签见下一节；`item tag batch` 仍走 Web。local 失败不自动换 Web/Connector/SQLite。

本地 key 仅在 Zotero 返回 remember=true 时按 profile + Server ID 缓存；不进入输出。不要主动读取或展示授权文件。

### 7b. 本地单条标签（Zotero 10+）

单条 `item tag add/remove` 可显式走 Local API。不带 `--write-backend local` 时仍立即走 Web，不是预览。`item tag batch` 仍只走 Web，不要用它代替一条已知 item key 的本地标签。

1. 先 doctor。`local_api_write.supported` 只说明协议，`verified=false` 不是写入通过。用八位 item key，不要猜 key，也不要读 Web 凭据来完成本地路径。
2. `item tag add KEY --tag NAME --write-backend local` 或 `item tag remove KEY --tag NAME --write-backend local` 默认只预览。可重复 `--tag`。
3. 新标签是手工标签（type 0）。精确同名已存在则 unchanged，不改变原 type，包括自动标签。移除按精确名称移除该名称的全部关联，包括同名不同 type；其他标签对象和未知属性保留。
4. 不做大小写、Unicode 或同义词折叠，不修剪非空名称。纯空白拒绝。中文、空格和特殊字符按输入保留。
5. 用户认可具体 before/after 后，保持原参数并增加 `--confirm --expected-server-id <预览值> --expected-version <预览值>`。预览和无变化不授权、不 PATCH。不要替用户点击授权框。
6. 只有 applied 且 verified 才表示确认成功。unknown 或 verification-failed 不能称为成功，也不能盲重试。冲突须重新预览。
7. 只作用于同库、非回收站的顶层普通条目。不支持标签颜色、类型转换、重命名，也不通过 `--field tags` 绕过。

### 7c. 本地分类创建（Zotero 10+）

`collection create NAME [--parent KEY] --write-backend local` 默认只预览。不带 local 时仍立即走 Web。重命名、删除和重挂父级仍只走 Web。

1. 先 doctor。用完整分类列表确认父分类属于同一库；不要猜 key，也不要一次创建多层路径。
2. 预览展示名称、父级和库版本。确认使用 `--confirm --expected-server-id <预览值> --expected-library-version <预览值>`。不要把不存在对象的 version 填 0，也不要复用条目的 `--expected-version`。
3. 同一父级精确同名则拒绝并列出候选 key。不复用、不合并、不重命名。不同父级可以同名。纯空白名称拒绝。
4. HTTP 200 不等于成功。只有对象级成功且回读 name、parent、server 一致才是 applied。unknown 不自动重发，也不把事后同名分类认作这次创建。
5. 创建不改成员、children 或附件。得到的 key 只作为现有 `collection add-item --write-backend local` 的输入。

### 7d. 本地批量整理（Zotero 10+）

不同条目的不同标签/分类决策走 `library organize plan/apply --write-backend local`，不要用 `item tag batch`，也不要在批次里创建分类。

1. 先用只读命令解析八位 item key 和已有 collection key。reason 只给人看，不执行其中的路径、URL 或脚本。
2. `library organize plan --file requests.json --output plan.json --write-backend local` 只读预览。核对完整 before/after、版本和分类名称。SHA-256 在命令 JSON 里，不在计划文件里。
3. 默认最多改变 50 条。超限零写入。要扩大必须 plan 与 apply 使用同一个显式 `--max-affected` 并重新审阅。
4. 用户认可后，`apply --plan plan.json --confirm --expected-plan-sha256 <命令输出> --report report.json --write-backend local`。hash、scope、profile 或版本不符时不要写入。
5. 每条目最多一次条件 PATCH。首次 failed、conflict、unknown 或 verification-failed 即停止，后续是 not-attempted。不自动回滚，不盲重试，不自动 resume。
6. 只有全部 applied 或 unchanged 且报告成功落盘才算批次成功。partial、failed、unknown 即使 envelope `ok: true` 也不是全成功，退出码非 0。落盘失败时 stdout 保留已知结果及报告错误；磁盘 in-flight 是待核查，不得重放。

## 安全门

默认视为有副作用的动作：

- `item create`
- `item add-doi`
- `item add-url`
- `item add-file`
- `item import --confirm`
- `item update`
- `item trash`
- `item restore`
- `item attach`
- `item merge --confirm`
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
- `library dedupe --confirm`
- `sync update-status --apply`
- `config init`
- `config set`
- `config profiles use`

执行规则：

1. 用户只是“看看”“分析”“评估”时，不要偷偷写库。
2. 普通、单项、可逆写操作，在用户明确要求后可以执行。
3. 高风险动作分三层，先总结即将发生的变化、再确认、再执行：

   层 A，可逆软删除（仍要确认，但稳态可恢复）：
   - `item trash` / `item restore`
   - `item note delete`（实际是把 note 移到 trash，文案 `Note moved to trash`）

   层 B，高风险删除、合并或状态写入：
   - `collection delete`
   - `library saved-search delete`
   - `library duplicates-merge --confirm`
   - `library dedupe --confirm`
   - `item merge --confirm`
   - `sync update-status --apply`

   层 C，批量写（影响一组条目，必须先在小范围试，再放开）：
   - `item import --confirm`（先 dry-run 复述目标可写性、记录数和格式）
   - `item tag batch --add-tag/--remove-tag`（不带 `--confirm` 只做本地 preview；核对
     `matched`、`affected`、`truncated`、`sample_keys` 和 `exceeds_max_affected` 后，用完全相同
     的 filters/mutations 加 `--confirm`；超过默认 50 条时必须在复核后显式提高
     `--max-affected`）
   - `library duplicates-merge`（多源 → 单 keeper）
   - `library dedupe --confirm`（整库/整 collection 多组批量合并，先用 `--collection` 圈小范围、复查 low-confidence 组）

4. `item merge` / `item tag batch` / `library duplicates-merge` / `library dedupe` / `sync update-status` 不带 `--confirm` / `--apply` 时本身就是 dry-run preview；要把 preview 当成“还没改”，不要错说成“已经合并 / 已经写回”。
5. connector import 的 preview 与 confirm 必须保持同一输入和格式，并在 confirm 前重新检查当前目标可写性；connector 失败不能改走 Web。
6. merge/dedupe 的 preview 是本地只读规划，confirm 只走 Web API；缺少 Web 凭据时保留 preview 结果并停止，不能改走 connector。
7. `library dedupe` 的 low-confidence 组默认跳过。不要把普通 confirm 当作授权，也不要自行追加 `--include-low-confidence`；必须先单独展示这些组，再取得一次明确的风险授权。
8. 写权限缺失或目标路径不支持该 mutation 时停在只读分析，不要假装成功。
9. `item tag batch --confirm` 返回 `state: applied|partial|failed` 和逐操作结果。只要
   `failed_operations > 0` 就必须明确报告失败项；不能因命令返回了 envelope 就称为全部成功。
10. `item tag add/remove` 不带 `--write-backend local` 时立即走 Web API。显式 local 时默认只预览；确认必须带预览中的 Server ID 和条目 version。`item tag batch` 不因单条 local 标签或 `library organize` 而改变，仍是 Web 批量命令。

### preprint 状态同步

用户通常会说：“preprint 的正式发表信息有没有要更新”。

- `sync update-status [<key>] [--collection K] [--limit N] [--apply]`
- 依赖 Zotero Web API 凭据，先 `doctor`
- 默认 dry-run；只有加了 `--apply` 才真把字段写回 Zotero
- 当前只覆盖 preprint publication status，不要把它当成附件索引器
- 版本增量和回收站枚举是读取，见 [reads.md](reads.md)
