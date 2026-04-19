---
name: zot-skills
description: 当用户在 Claude Code、Codex 或类似 agent 里，想直接查询、提取、整理或安全更新本机已有的 Zotero 内容时，必须使用这个 skill。重点是 Zotero 里的 metadata、notes、tags、attachments、PDF fulltext、outline、annotations、collections、saved searches、feeds 和 reading workspace，而不是教人背 CLI。Rust `zot` CLI 只是执行层。适用于库内搜索、citation key 查询、批注与 PDF 提取、workspace 建立与检索、saved search 保存、附件下载、semantic index/search、Scite 检查、配置排障，以及明确授权的 Zotero Web API 写操作。不要把它用于泛化找论文、普通总结、引用格式教学、或不落到 Zotero / workspace 的 PDF 处理。
---

# zot-skills

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
- “看这个 collection 里有哪些条目”
- “列出当前库里的 feeds”

优先路由：

- 普通库内检索：`library search`
- citation key 直达：`library citekey`
- collection 细粒度读取：`collection get` / `subcollections` / `items` / `item-count` / `tags`
- 库级组织信息：`library tags` / `libraries` / `feeds` / `feed-items`

### 2. 取证据

用户通常会说：

- “把这篇文献的详情、children、引用拿出来”
- “把 PDF 批注、outline、note 都拉出来”
- “把附件下载到本地”

优先路由：

- 单篇主入口：`item get` / `related` / `children` / `cite` / `export`
- PDF 证据：`item pdf` / `fulltext` / `outline`
- annotation：`item annotation list` / `search`
- 附件下载：`item download <attachment-key>`

### 3. 建 workspace

用户通常会说：

- “给我建一个 llm-safety workspace”
- “把 mechanistic interpretability 相关论文整理进一个长期工作区”
- “后面我要在这个主题里做问答检索”

优先路由：

- 建与维护：`workspace new` / `add` / `import` / `remove` / `delete`
- 索引与查询：`workspace index` / `search` / `query`

注意：

- workspace 名必须是 kebab-case，例如 `llm-safety`
- `workspace search` 是关键词检索
- `workspace query` 是问答式检索

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
- “把这个条目挂到某个 collection”
- “合并重复条目”
- “把 preprint 的正式发表信息写回去”

优先路由：

- 条目与标签：`item note ...` / `item tag ...` / `item update`
- 导入：`item add-doi` / `add-url` / `add-file`
- collection 关系：`collection add-item` / `remove-item` / `create` / `rename` / `delete`
- duplicates：`library duplicates` / `duplicates-merge`
- 状态同步：`sync update-status`

### 7. 配置排障

用户通常会说：

- “为什么这个环境不能写 Zotero”
- “先帮我看看配置是不是对的”
- “把当前 profile 切到 work”
- “初始化一个新的 config profile”

优先路由：

- 诊断：`doctor`
- 配置：`config show` / `init` / `set` / `profiles list` / `profiles use`

## 调用顺序

1. 如果系统已安装 `zot`，优先用 `zot --json ...`
2. 只有在开发仓库环境且 `zot` 不在 `PATH` 时，才退回：

```bash
cargo run -q -p zot-cli -- ...
```

3. 同一轮任务保持同一种调用方式，不要来回切换。

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
- workspace 名必须是 kebab-case
- `zot mcp serve` 当前不可用
- `item add-file` 不支持 `--attach-mode`
- `item annotation create` / `create-area` 只适用于 PDF attachment
- `library saved-search` 处理的是保存查询的条件，不是结果项
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
3. 对这些高风险动作，先总结即将发生的变化，再执行：
   - `item trash`
   - `item note delete`
   - `collection delete`
   - `library saved-search delete`
   - `library duplicates-merge --confirm`
   - `sync update-status --apply`
4. 写权限缺失时停在只读分析，不要假装成功。

## 常见语义差异

- `workspace search` 是关键词检索，`workspace query` 是问答检索
- `library semantic-search` 是库级语义检索，不等价于 workspace query
- `item add-doi` / `item add-url` / `item create --doi|--url|--pdf` 支持 `--attach-mode`
- `item add-file` 可以带 `--doi` 补元数据，但不接受 `--attach-mode`
- feeds 不通过 `--library group:<id>` 访问，而是用 `library feeds` / `feed-items`
- `item download` 下载本地附件文件，`item attach` 上传新附件
- `config show` 是看有效配置，`config profiles use` 是切换默认 profile

## 自然语言到动作的典型映射

- “找我库里 reward hacking 相关的论文，再挑一篇最相关的给我引用”  
  先 `library search`，再 `item get` / `item cite`

- “把这篇论文的 PDF 批注和 notes 拉出来”  
  先 `doctor`，再 `item get` / `item children` / `item annotation list`

- “给我建一个 llm-safety workspace，后面我要做问答”  
  先 `workspace new` / `import`，再 `index` / `query`

- “把这个筛选条件存成保存查询”  
  走 `library saved-search create`

- “把附件 ATCH005 下载出来”  
  走 `item download`

- “我现在这个环境为什么不能写 Zotero”  
  先 `doctor`，必要时 `config show`

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

这个 skill 的目标不是把 CLI 解释得更完整，而是让 Claude Code、Codex 等 agent 用自然语言稳定完成 Zotero 工作流。
