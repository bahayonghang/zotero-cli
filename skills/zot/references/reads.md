# 读取、检索与证据

入口已经选定只读路径。按这里的命令执行；不要把这些命令改成写入，也不要编造不存在的子命令。

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
  - 非空 search 的 `meta.fulltext_index` 为 `legacy-tables` / `fts5-sidecar` / `unavailable`。`unavailable` 表示本次未查附件全文，标题/作者/标签仍可能命中。不要把 `no such table: fulltextItemWords` 当成 SQLite 不可读。
  - 标题、期刊、主题是字段条件：`--title`、`--journal`（只匹配 `publicationTitle` 或 `journalAbbreviation`）、`--topic`（标题、摘要或标签的词匹配）。主题检索不是语义召回，也不读 PDF 正文。 语义召回用 `library semantic-search`，它不是 `--topic`，也不等于 `workspace query`。
  - 默认位置参数仍会查附件全文。Zotero 占着库锁时，这条查询改读正在运行的桌面，并用 `meta.fulltext_index=unavailable` 说明没查正文。空结果不表示正文里没有这个词。
  - 唯一标题才直接读单条。多条候选先列出来让用户选，不要把第一页第一条当成那一篇。
  - 成功时 `meta.read_source` 为 `desktop`、`desktop-unbound` 或 `snapshot`。回答要说明这次用的来源。
  - 配置了 profile 或明确数据目录、又还没证明和正在运行的 Zotero 是同一个库时，读运行中的实例要加 `--running-zotero`。默认不要换成另一套库。
- 纯列表 / 翻页：`library list --collection ... --limit ... --offset ...`
  - Zotero 打开且本机通信可用时走桌面。先取完整过滤结果，在本地按条目 key 降序，再应用 `offset` / `limit`。不要向 Zotero 发送 `sort=key`。
  - `--collection` 接受 key，或唯一的分类名。重名返回 `collection-ambiguous`，候选 key 按升序给出。名称解析使用同一次分类列表，不另开快照。
  - 默认排除附件、笔记、注释和回收站。`--include-trashed` 与 `meta.trash_policy` 保持原样。
- 整库统计：`library stats`
  - `top_tags`、`total_items`、`by_type`、`collections` 只统计书目条目。`pdf_attachments` 是拥有 `application/pdf` 的不同书目父项数，同一父项两份 PDF 只算 1。`notes` 是书目父项下的笔记行数，没有书目父项的笔记算 0。后两个字段的回收站看父项，不看子项自己。
- citation key 直达：`library citekey`
  - 精确匹配，区分大小写。前缀相似不是命中。
  - 关闭的库同时查原生 `citationKey` 和 Extra 里的 `Citation Key:` / `CitationKey:`。
  - 打开的库先用 Local API 字段和 Extra。插件候选只有在同一库条目上核对到这个 key 时才接受。插件不可达、而且库里没存过这个 key 时，说明来源限制，不要说成确定没有。
  - 多条条目共用同一个 key 时停下来让用户选，并给出标题、作者、年份和 item key。
- 最近入库条目：`library recent --count`
- collection 细粒度读取：`collection list` / `get` / `subcollections` / `items` / `item-count` / `tags` / `search`
  - 这七条和 `library list` / `stats` / `tags` 在 Zotero 打开且本机通信可用时走桌面。数据目录还不能证明是同一个库时加 `--running-zotero`。成功的 `meta.read_source` 为 `desktop` 或 `desktop-unbound`。人类输出不另起 `Source:` 行。
  - `items`、`item-count`、`tags` 使用同一组直接成员：`/collections/{key}/items/top?includeTrashed=1`。子笔记和子附件不算成员。不请求 `/collections/{key}/tags`。
  - 桌面分类树省略回收站分类，`meta.trashed_collections` 为 `omitted`。桌面成员省略直接注释，`meta.direct_annotations` 为 `omitted`。关闭时的 SQLite 列表仍返回那些分类，不为了补齐而打开另一边。
- collection 名内检索：`collection search`
- 库级组织信息：`library tags` / `libraries` / `feeds` / `feed-items`
  - `library tags` 统计该库全部条目标签，包含笔记、附件、注释和回收站。它和 `library stats` 的 `top_tags` 不是同一口径。顺序是计数降序、名称升序。

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
- 目标已存在时先报告冲突，不要自行追加 `--force`

### 9. 撤稿与引用质量（Scite）

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

### 10. 同步与增量

用户通常会说：

- “看我库里哪些条目自版本号 N 之后变过”
- “回收站现在有什么”

优先路由：

- 远端版本增量：`item versions --since <number>`
- 回收站枚举：`item deleted --limit N`

这两个命令都依赖 Zotero Web API 凭据，先 `doctor`。preprint 状态写回不是读取，走 [writes.md](writes.md) 的 `sync update-status`。

## 自然语言到动作的典型映射

与上面各节重复的例子不再单列。这里只保留旧映射里没有写进其他小节的限制。

- “把 citation key 为 vaswani_attention_2023 的文献插进草稿，并维护 references.bib”：先 `library citekey` 解析 Zotero item key，再用 `item cite --style apa` 取显示引用、`item export --format bibtex` 取 BibTeX。agent 可以更新 `references.bib` 和草稿中的 BibTeX citation key。CLI 不直接编辑草稿或 `.bib` 文件。
