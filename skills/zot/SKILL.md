---
name: zot
description: 当用户在 Claude Code、Codex 或类似 agent 里，想直接查询、提取、整理或安全更新本机已有的 Zotero 内容时，必须使用这个 skill。重点是 Zotero 里的 metadata、notes、tags、attachments、PDF fulltext、outline、annotations、collections、saved searches、feeds 和 reading workspace，而不是教人背 CLI。Rust `zot` CLI 只是执行层。适用于库内搜索、citation key 查询、批注与 PDF 提取、workspace 建立与检索、saved search 保存、附件下载、semantic index/search、Scite 检查、配置排障，以及明确授权的 Zotero Web API 写操作。不要把它用于泛化找论文、普通总结、引用格式教学、或不落到 Zotero / workspace 的 PDF 处理。
---

# zot

这个 skill 的目标不是展示命令面，而是把用户的自然语言 Zotero 任务稳稳落到正确的运行时路径，再把真正有用的结果带回来。

## 先抓住这几个原则

- 只要任务的真实目标是操作**已有的本地 Zotero 库**或**已有/将创建的 reading workspace**，就用本 skill，即使用户没说 `zot`。
- 用户在 Claude Code、Codex 里应该直接说需求，不应该先背命令。skill 负责把需求翻成运行时动作。
- `zot` 是唯一执行面。`ref/` 里的旧 Python 参考实现和 `zot mcp serve` 都不是当前主路径。
- 回答先给结论、证据、变更或失败原因。不要先把 raw JSON 倒给用户。

## 先按意图分桶

### 1. 查条目

用户通常会说：

- “找我库里 reward hacking 相关的论文”
- “按 Smith2024 找到那篇论文”
- “给我看最近 10 条刚进库的文献”
- “看这个 collection 里有哪些条目”
- “列出当前库里的 feeds”
- “翻页看第 100-150 条”
- “哪个 collection 名字里含 transformer”
- “给我整库的条目数和按类型分布”

优先路由：

- 普通库内检索：`library search`
- 纯列表 / 翻页：`library list --collection ... --limit ... --offset ...`
- 整库统计：`library stats`
- citation key 直达：`library citekey`
- 最近入库条目：`library recent --count`
- collection 细粒度读取：`collection get` / `subcollections` / `items` / `item-count` / `tags`
- collection 名内检索：`collection search`
- 库级组织信息：`library tags` / `libraries` / `feeds` / `feed-items`

### 2. 取证据

用户通常会说：

- “把这篇文献的详情、children、引用拿出来”
- “把 PDF 批注、outline、note 都拉出来”
- “把附件下载到本地”
- “在我所有 note 里搜 reward shaping”
- “看这条现在打了哪些 tag”
- “用浏览器打开它的 DOI”
- “用本地 PDF reader 打开这篇的附件”

优先路由：

- 单篇主入口：`item get` / `related` / `children` / `cite` / `export`
- PDF 证据：`item pdf` / `fulltext` / `outline`
- annotation：`item annotation list` / `search`
- note 关键词检索：`item note search`
- tag 只读列：`item tag list`
- 打开本地资源：`item open`（附件）/ `item open --url`（DOI 或 URL）
- 附件下载：`item download <attachment-key>`

### 3. 建 workspace

用户通常会说：

- “给我建一个 llm-safety workspace”
- “把 mechanistic interpretability 相关论文整理进一个长期工作区”
- “后面我要在这个主题里做问答检索”
- “看 llm-safety workspace 里现在有哪些条目”
- “把这个 workspace 导出成 BibTeX / markdown 给我贴邮件”

优先路由：

- 建与维护：`workspace new` / `add` / `import` / `remove` / `delete`
- 查看成员：`workspace list` / `workspace show <name> --limit N`
- 索引与查询：`workspace index` / `search` / `query`
  - 0.5.0 起默认增量重索引（仅 embed 新增项）。需要全重建时加 `--force-rebuild`；跳过 PDF 全文加 `--no-fulltext`。
- 导出：`workspace export <name> --format markdown|json|bibtex`

注意：

- workspace 名必须是 kebab-case，例如 `llm-safety`
- `workspace search` 是关键词检索
- `workspace query` 是问答式检索
- `workspace export --format bibtex` 会逐条调本地 BibTeX 导出，没有 BibTeX 数据的条目会被跳过

### 4. 保存查询

用户通常会说：

- “把这个筛选条件存成一个 Zotero saved search”
- “列出我现在有哪些保存查询”
- “删掉这个过期的 saved search”

优先路由：

- `library saved-search list`
- `library saved-search create`
- `library saved-search delete`

边界：

- Zotero Web API 当前只提供 saved search 的元数据和条件，不直接返回搜索结果
- 要解释保存的是“查询条件”，不是“动态结果集快照”

### 5. 下载附件

用户通常会说：

- “把 ATCH005 这个附件下载出来”
- “把这篇条目的 PDF 拉到当前目录”

优先路由：

- 已知 attachment key：`item download`
- 只知道父条目时：先 `item children`，再确定 attachment key

不要做的事：

- 不要把附件下载伪装成 `item attach`
- 不要把上传和下载混成一个动作

### 6. 安全写入

用户通常会说：

- “给这篇文献加一条 note”
- “打上 priority 标签”
- “把所有 reading-list 标签的条目都加上 priority”
- “把这个条目挂到某个 collection”
- “先预览再合并这两篇文献”
- “合并重复条目”
- “把 preprint 的正式发表信息写回去”

优先路由：

- 单条 note：`item note add` / `update` / `delete`
- 单条 tag：`item tag add` / `tag remove`
- 批量 tag（按搜索结果一次改多条，是高风险写）：`item tag batch --query/--tag --add-tag/--remove-tag`
- 元数据更新：`item update --field name=value`
- 导入：`item add-doi` / `add-url` / `add-file`
- 手工合并（不带 `--confirm` 是 dry-run preview）：`item merge`
- collection 关系：`collection add-item` / `remove-item` / `create` / `rename` / `delete`
- duplicates（不带 `--confirm` 是 dry-run）：`library duplicates` / `duplicates-merge`
- 状态同步（不带 `--apply` 是 dry-run）：`sync update-status`

### 7. 配置排障

用户通常会说：

- “为什么这个环境不能写 Zotero”
- “先帮我看看配置是不是对的”
- “把当前 profile 切到 work”
- “初始化一个新的 config profile”

优先路由：

- 诊断：`doctor`
- 配置：`config show` / `init` / `set` / `profiles list` / `profiles use`

### 8. 撤稿与引用质量（Scite）

用户通常会说：

- “这篇有没有被撤稿”
- “在我库里整体扫一遍 retraction notice”
- “看 attention 主题相关条目里有没有撤稿或勘误”

优先路由：

- 单条：`item scite report --item-key K` 或 `item scite report --doi 10.x/y`
- 库内多条：`item scite search <query>`（按库内已有条目的 DOI 批量查 Scite）
- 整库扫撤稿：`item scite retractions [--collection K] [--tag T] --limit N`

注意：

- Scite 报告依赖外部 Scite 服务；外部网络不可用时直说，不要伪造结果
- `item scite report` 必须给 `--item-key` 或 `--doi` 之一

### 9. 同步与增量

用户通常会说：

- “看我库里哪些条目自版本号 N 之后变过”
- “回收站现在有什么”
- “preprint 的正式发表信息有没有要更新”

优先路由：

- 远端版本增量：`item versions --since <number>`
- 回收站枚举：`item deleted --limit N`
- preprint 状态同步（dry-run vs `--apply`）：`sync update-status [<key>] [--collection K] [--limit N] [--apply]`

注意：

- 这三个命令都依赖 Zotero Web API 凭据，先 `doctor`
- `sync update-status` 默认 dry-run；只有加了 `--apply` 才真把字段写回 Zotero
- `sync update-status` 当前只覆盖 preprint publication status，不要把它当成附件索引器

## 调用顺序

1. 如果系统已安装 `zot`，优先用 `zot --json ...`
2. 只有在开发仓库环境且 `zot` 不在 `PATH` 时，才退回：

```bash
cargo run -q -p zot-cli -- ...
```

3. 同一轮任务保持同一种调用方式，不要来回切换。
4. agent 模式下默认用 `--json` 拿 envelope，文本模式只面向真人；同一会话不要混用。

## 诊断门

以下场景默认先跑 `doctor`：

- 第一次接触这个环境
- 任何写操作
- PDF / outline / annotation / attachment 相关任务
- semantic index / semantic search / workspace query
- citation key 查询
- saved search / 配置排障 / profile 切换
- 用户说“为什么不工作”

首选：

```bash
zot --json doctor
```

开发环境 fallback：

```bash
cargo run -q -p zot-cli -- --json doctor
```

重点看这些字段：

- `db_exists`
- `write_credentials.configured`
- `pdf_backend.available`
- `better_bibtex.available`
- `libraries.feeds_available`
- `semantic_index`
- `annotation_support`
- `embedding.configured`
- `config_file`

## 硬约束

- `--library` 只接受 `user` 或 `group:<id>`
- `--json` 是 global flag，必须放在子命令前（例 `zot --json item get K`），不能写成 `zot item --json get K`
- workspace 名必须是 kebab-case
- workspace 文件实际位置：`~/.config/zot/workspaces/<name>.toml`，索引副文件 `<name>.idx.sqlite`，PDF cache 副文件 `.md_cache.sqlite`
- `zot mcp serve` 当前不可用
- `item add-file` 不支持 `--attach-mode`
- `item annotation create` / `create-area` 只适用于 PDF attachment，且 attachment 的 `content_type` 必须是 `application/pdf`，否则报 `attachment-not-pdf`
  - `item annotation create` 支持 `--occurrence N`（0.5.0 起，默认 1），用于在同一页出现多次的同一文本中选中第 N 个。返回 JSON 含 `occurrence` / `total_matches` / `more_occurrences`，可用于连锁调用。
- `library saved-search` 处理的是保存查询的条件，不是结果项
- `library saved-search create --conditions` 必须是 JSON 数组，每项形如 `{"condition": "...", "operator": "...", "value": "..."}`，至少一条；空数组直接报 `saved-search-conditions`
- Pdfium 路径覆盖：`ZOT_PDFIUM_LIB_PATH` / `PDFIUM_LIB_PATH` 指向 lib，`ZOT_PDFIUM_CACHE_DIR` 覆盖自动下载缓存目录
- 永远不要直接修改 `zotero.sqlite`

## 安全门

默认视为有副作用的动作：

- `item create`
- `item add-doi`
- `item add-url`
- `item add-file`
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

   层 B，不可逆远程写：
   - `collection delete`
   - `library saved-search delete`
   - `library duplicates-merge --confirm`
   - `item merge --confirm`
   - `sync update-status --apply`

   层 C，批量写（影响一组条目，必须先在小范围试，再放开）：
   - `item tag batch --add-tag/--remove-tag`
   - `library duplicates-merge`（多源 → 单 keeper）

4. `item merge` / `library duplicates-merge` / `sync update-status` 不带 `--confirm` / `--apply` 时本身就是 dry-run preview；要把 preview 当成“还没改”，不要错说成“已经合并 / 已经写回”。
5. 写权限缺失时停在只读分析，不要假装成功。

## 常见语义差异

- `workspace search` 是关键词检索，`workspace query` 是问答检索
- `library recent --count 10` 是最近 N 条，`library recent 2026-04-01 --limit 20` 是按时间边界筛
- `library semantic-search` 是库级语义检索，不等价于 workspace query
- `item add-doi` / `item add-url` / `item create --doi|--url|--pdf` 支持 `--attach-mode`
- `item add-file` 可以带 `--doi` 补元数据，但不接受 `--attach-mode`
- feeds 不通过 `--library group:<id>` 访问，而是用 `library feeds` / `feed-items`
- `item download` 下载本地附件文件，`item attach` 上传新附件
- `item merge` 是手工选 keeper/source 的通用合并，`library duplicates-merge` 是先找重复、再按 keeper 合并
- `config show` 是看有效配置，`config profiles use` 是切换默认 profile

## 自然语言到动作的典型映射

- “找我库里 reward hacking 相关的论文，再挑一篇最相关的给我引用”  
  先 `library search`，再 `item get` / `item cite`

- “给我看最近 10 条刚进库的文献”  
  走 `library recent --count 10`

- “把这篇论文的 PDF 批注和 notes 拉出来”  
  先 `doctor`，再 `item get` / `item children` / `item annotation list`

- “给我建一个 llm-safety workspace，后面我要做问答”  
  先 `workspace new` / `import`，再 `index` / `query`

- “把 llm-safety workspace 导出成 BibTeX 给我贴邮件”  
  先 `workspace show llm-safety`，再 `workspace export llm-safety --format bibtex`

- “把这个筛选条件存成保存查询”  
  走 `library saved-search create`

- “把附件 ATCH005 下载出来”  
  走 `item download`

- “用浏览器打开 ATTN001 的 DOI 看看”  
  走 `item open ATTN001 --url`

- “先预览再合并 KEEP001 和 DUPE001，确认后再真的合并”  
  先 `item merge KEEP001 DUPE001`，确认后再 `item merge ... --confirm`

- “我库里这些跟 attention 有关的条目，挨个查一下 Scite 报告”  
  走 `item scite search "attention"`

- “看 KG326EEI 这条最近改没改”  
  走 `item versions --since <last-version>`

- “把 Recent RL 这个 saved search 删掉”  
  走 `library saved-search delete <key>`

- “我现在这个环境为什么不能写 Zotero”  
  先 `doctor`，必要时 `config show`

## 从 ref\zotero-cli 迁移过来时怎么理解

- `search` -> `library search`
- `get` -> `item get`
- `open` -> `item open`
- `open --url` -> `item open --url`
- `annotations` -> `item annotation list` 或 `item pdf --annotations`
- `notes` -> `item note list`
- `notes search` -> `item note search`
- `collections` -> `collection list`
- `collection <id>` -> `collection items <id>`
- `add doi` / `add url` -> `item add-doi` / `item add-url`
- `tags` -> `library tags`
- `stats` -> `library stats`
- `recent [n]` -> `library recent --count <n>`
- `merge` -> `item merge`；如果是先找重复再合并，走 `library duplicates` / `duplicates-merge`

不要迁回去的旧心智：

- 不补 flat top-level alias
- 不补 `--api-base`
- 不补 compact JSON 默认输出
- 不把 connector 风格 `search` / `fetch` 重新做成另一套主命令

## 从 ref\zotagent 迁移过来时怎么理解

先记住两点：

- 当前 `zot` 没有照搬 `zotagent` 的 flat command 面
- 当前 `sync` 只做 preprint publication status，同名但不是附件索引器

已有覆盖或可替代的部分：

- DOI / URL / 文件导入：`item add-doi` / `item add-url` / `item add-file` / `item create`
- 库级语义检索：`library semantic-index` / `library semantic-search`
- 单篇 PDF 提取：`item pdf` / `item fulltext` / `item outline`
- citation key 入口：先 `library citekey`，再转到 `item get` / `item cite`
- 撤稿覆盖：`item scite retractions` / `item scite report` / `item scite search`（zotagent 通常没内置，这是 zot 多出来的能力，不是迁移）
- 回收站枚举与版本增量：`item deleted` / `item versions --since`（替代 zotagent 风格的轮询脚本）

当前没补齐，不能假装存在：

- `s2`
- `add --s2-paper-id`
- `search-in`
- `metadata`
- `read`
- `expand`
- zotagent 风格 `status`
- zotagent 风格 `sync` 全量附件索引
- 按 `title` / `author` / `year` / `publication` 一次性手工建条目

遇到这些请求时怎么处理：

- `search-in` / `expand`：明确说当前没有等价命令，只能先 `item fulltext` / `item pdf` 拉文本，再做 agent 侧二次定位
- `metadata`：说明当前没有 field-scoped metadata search；最多退到 `library search` 加已有 filter
- `status`：用 `doctor` + `library semantic-status` 组合回答，不要把 `sync update-status` 说成索引状态
- `s2` / `--s2-paper-id`：直接说明当前未实现，不要伪造替代命令
- zotagent `sync`：说明当前只能用 `library semantic-index --fulltext` 或 `workspace index` 做部分替代，而且范围主要是 metadata + PDF

## 失败时的 fallback

- 没有 `zot`：开发仓库里退回 `cargo run -q -p zot-cli -- ...`
- 没有写权限：明确告诉用户缺 `ZOT_API_KEY` / `ZOT_LIBRARY_ID`
- 没有 Better BibTeX：`library citekey` 只走 Extra fallback
- 没有 Pdfium：不要承诺 fulltext / outline / annotation / PDF 下载后的文本处理
- 没有 embedding：semantic 检索说明会降级；workspace 问答改用 `--mode bm25`
- `attach-mode auto` 没找到 OA PDF：条目仍可能创建成功

## 输出契约

最终回答应该：

- 先回答用户真正的问题，而不是先贴命令
- 再给关键证据、已执行动作或失败原因
- 如果失败，明确告诉用户缺什么、下一步是什么
- 默认不要倾倒 raw JSON；先读 envelope，再转述有效信息
- 优先复述 envelope 里 `data` / `meta` 的关键字段，把 raw JSON 当二级证据，必要时再贴

这个 skill 的目标不是把 CLI 解释得更完整，而是让 Claude Code、Codex 等 agent 用自然语言稳定完成 Zotero 工作流。
