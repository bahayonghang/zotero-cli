---
name: zot
description: 当用户要直接查询、提取、整理或安全更新本机已有的 Zotero 库或 reading workspace 时使用，即使用户没说 zot。覆盖库内搜索、citation key、PDF/批注/笔记、collection、saved search、feed、附件、语义索引、Scite、配置排障、Zotero 10+ Local API 字段/作者/分类创建与单 key 删除/单条标签、connector 导入 BibTeX/RIS，以及明确授权的 Web API 写入。不要用于泛化找论文、引用格式教学、非 Zotero PDF，也不要用于文献缺陷分析、创新点或 brainstorm 报告；后者使用 zot-brainstorm。 (v1.1.0)
---

# zot

把自然语言 Zotero 任务落到正确的运行时路径，再返回结论、证据、变更或失败原因。不要先倾倒 raw JSON，也不要教用户背命令。

## 执行原则

- 目标是已有本地 Zotero 库或 reading workspace 时使用本 skill。文献缺陷、创新点或研究报告交给 `zot-brainstorm`。
- `zot` 是唯一执行面。`ref/` 和 `zot mcp serve` 都不是当前路径。
- SQLite 只读。Zotero 10+ Local API 只做已实现的字段、作者、分类创建与成员、单 key 分类删除和单条标签操作。Connector 只新增导入 BibTeX/RIS。其他 mutation 走 Web API。
- 打开的库中，`item get`、`item children`、`item pdf` / `fulltext`、`library citekey`、`collection list` / `get` / `subcollections` / `search` / `items` / `item-count` / `tags`，以及 `library list` / `stats` / `tags` 可以读正在运行的桌面。非空通用 `library search` 仍先读快照。一次命令只用一个来源，不拼接桌面结果和快照，也不自动改走 Web API。
- 每条写路径拥有自己的成功和失败。Connector 或 Web API 出错时不能 fallback 到另一条路径。
- 旧命令名或未实现能力先读 [references/migration.md](references/migration.md)，不要编造命令。

## 先按意图分桶

每个桶只决定首选命令和停止点。命令细节在链接的参考文件里。

1. 查条目：`library search`、`list`、`stats`、`recent`、`citekey`，或 collection、tag、feed 读取。多条候选或同 key 先让用户选。字段、来源和打开库语义读 [references/reads.md](references/reads.md)。
2. 取证据：`item get`、children、cite、export、PDF、annotation、note、tag 或 `item open`。先解析出唯一 item key 或 attachment key。细节读 [references/reads.md](references/reads.md)。
3. Workspace：`workspace new/add/import/remove/delete/list/show/index/search/query/export`。名称必须是 kebab-case；`search` 是关键词，`query` 是问答。细节读 [references/reads.md](references/reads.md)。
4. 保存查询：`library saved-search list/create/delete`。保存的是查询条件，不是结果快照。删除前进入下面的安全门。细节读 [references/reads.md](references/reads.md)。
5. 下载附件：已知 key 用 `item download`；只有父条目时先 `item children`。不要把它当成 `item attach`，目标已存在时不自行加 `--force`。细节读 [references/reads.md](references/reads.md)。
6. 导入：唯一路由是 `item import`。先 dry-run 并复述目标、可写性和记录数；同一输入确认后才加 `--confirm`。目标变化就停止。细节读 [references/writes.md](references/writes.md)。
7. 安全写入：先选后端，再按下一节预览和确认。字段、作者、分类创建与单 key 删除、单条标签、批量整理、merge/dedupe 和 preprint 状态都读 [references/writes.md](references/writes.md)。
8. 配置排障：`doctor` 或 `config show/init/set/profiles`。不要把 connector 配成通用写后端。字段含义读 [references/doctor.md](references/doctor.md)。
9. Scite：单条 `item scite report`，库内 `item scite search`，撤稿扫描 `item scite retractions`。外部服务失败就直说，不伪造结果。细节读 [references/reads.md](references/reads.md)。
10. 版本与回收站：`item versions --since` 和 `item deleted` 是读取。`sync update-status` 是 Web 写入，默认 dry-run。读取细节在 [references/reads.md](references/reads.md)，状态写回在 [references/writes.md](references/writes.md)。

## 调用顺序

1. 已安装 `zot` 时只用 `zot --json ...`。
2. 只有在本开发仓库且 `zot` 不在 `PATH` 时，才改用 `cargo run -q -p zot-cli -- --json ...`。
3. 同一轮任务保持同一种调用方式，也不要混用 JSON envelope 和文本模式。

## 诊断

第一次接触环境、任何写入、PDF 或附件排障、semantic index/search、workspace query、citation key、saved search、profile 切换，以及“为什么不工作”，先运行 `zot --json doctor`。

`item get`、`item children`、`item pdf`、`library search`、`library citekey`，以及 `collection list` / `get` / `subcollections` / `search` / `items` / `item-count` / `tags` 和 `library list` / `stats` / `tags` 不必先跑完整 doctor。SQLite snapshot 被锁时，这十条分类和库清单命令继续走桌面读，不要先让用户关闭 Zotero。非空通用 `library search` 仍先读快照。看 `capabilities.sqlite_snapshot` 和 `capabilities.desktop_read` 的 `next`。探针可达不等于条目或 PDF 已读。

路由时重点看四项能力：`desktop_read`、`local_api_write`、`connector_write`、`web_write`。`verified: false` 不是写入成功。其余字段、平台路径、硬约束和失败后的停止方式读 [references/doctor.md](references/doctor.md)。

## 安全

- 用户只是查看、分析或评估时，不写 Zotero。
- 预览不是完成。未带确认参数的 merge、dedupe、batch、import 和 status sync 都还没改库。
- 确认参数必须来自同一次预览，并保持同一输入、过滤器和目标。目标、版本、hash、scope 或 profile 变化时重新预览。
- 本地 `collection delete` 的确认必须同时带同一次预览的 Server ID、分类对象 version 和子孙 SHA-256。每次 DELETE 前会再读分类列表；父分类 version 未变但子孙 key 变了就拒绝，不发送 DELETE。有子分类时先说明它们会被永久删除，文献仍留在库里。Zotero 10.0.8 的分类列表不含废纸篓中的子分类，预览和哈希也看不到它们，但这次删除仍会永久擦除它们。不要替用户点击授权框。
- low-confidence 默认跳过。普通 confirm 不是授权，不能自行追加 `--include-low-confidence`。
- 批量写入先看影响范围，默认不超过 50 条。超过时必须显式提高上限并重新审阅。部分失败不能说成全部成功。
- 权限缺失、目标只读或路径不支持该 mutation 时停在只读说明。不改走另一条后端，不直接写 `zotero.sqlite`。
- 命令清单、三层风险、Local API 条件和批量整理流程读 [references/writes.md](references/writes.md)。

## 输出契约

- 先回答用户的问题，再给关键证据、已执行动作或失败原因。
- 区分 Zotero item key 和 BibTeX citation key，不能把两者当成同一个 key。
- 失败时点名具体门和下一步：Zotero 未运行、connector 不可达、目标只读、Web 凭据缺失、无匹配、来源限制或写未确认。
- 先读 envelope 的 `data` 和 `meta`。说明 `read_source` 和 `fulltext_index`，不要把不可用正文说成已经查过。
