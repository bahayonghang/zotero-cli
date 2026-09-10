# library search 兼容 Zotero 7.1+ 全文索引拆分

## Goal

让 `zot library search <query>` 在 Zotero userdata ≥127 上对字段、作者、标签匹配成功返回，并在 `data_dir/fulltext.sqlite` 可用时用 FTS5 继续匹配附件全文。doctor 与非空 search 的 JSON envelope 必须报告本次实际使用的全文后端，避免 agent 把 metadata 命中当成 PDF 全文命中。

## Background

- 2026-09-10 本机：`zot --json doctor` 中 `local_sqlite_read.available=true`、`schema_version=129`；`zot --json library search IEEE --limit 3` 返回 `search-count` / `no such table: fulltextItemWords`；`library list --limit 1` 成功，`meta.total=1765`。
- 本机 `C:\Users\lyh\Zotero\zotero.sqlite` 只有 `fulltextItems`，没有 `fulltextItemWords` / `fulltextWords`。同目录 `fulltext.sqlite` 约 119 MB，FTS5 虚表 `fulltextContent` / `fulltextContentCJK` / `fulltextNotes*`。主库 `version.userdata=129`。
- Zotero `schema.js` userdata **127** 删除两张词表。`fulltext.js` 把附件正文放进 ATTACH 库 `fulltext.sqlite`，`rowid = attachment itemID`，`MATCH` 查 `fulltextContent`（非 CJK）或 `fulltextContentCJK`（纯 CJK 2-gram）。
- `LocalLibrary::search` 在 `query` 非空时把四个 EXISTS 用 `OR` 绑成一条 WHERE（`src/zot-local/src/db.rs:226-235`）。`search-count` 的 `COUNT(*)` 在 prepare 时解析全部表名（`:281-286`）。空 query 的 list 不进入该 JOIN。
- `LocalLibrary::open` 只 Backup `zotero.sqlite`（`:1622-1661`）。测试富夹具仍是 userdata **120** 并创建词表（`:2985-2986`、`:3005-3006`、`:3165-3170`）。
- 可选表先例：`itemAnnotations` 缺失返回空列表（`:558-560`），`table_exists`（`:2190-2199`）。`.trellis/spec/zot-local/backend/error-handling.md` 要求缺失的可选 Zotero 表按版本兼容处理。
- 2026-09-10 用户确认：本任务纳入 `fulltext.sqlite` FTS5 附件全文，不另开子任务。`07-26-fix-db-semantics-perf` 曾把 FTS5 列为 Out Of Scope，本任务覆盖该限制。

## Requirements

### R1 非空 query 不得依赖已删除的词表

- `itemData` / `itemCreators` / `itemTags` 三个 EXISTS 在词表缺失时仍可 prepare 并执行。
- 两张词表都存在时，保留当前附件词表 LIKE 分支（`src/zot-local/src/db.rs:232`）：`itemAttachments` → `fulltextItemWords` → `fulltextWords`，`parentItemID = i.itemID`，`w.word LIKE ? ESCAPE '\'`。
- 任一词表缺失时省略该 EXISTS，不得 prepare 含这两张表名的语句，不得靠捕获 `no such table` 再跑同一条 SQL。
- 探测复用 `table_exists`。主条目过滤、trash 默认排除、OR 查询 / AND 过滤、`escape_like`、SQL `COUNT` + 页内 hydration 保持现有合同。

### R2 全文后端必须可观测

- 非空 `library search` 的 JSON `meta.fulltext_index` 取值仅为：
  - `legacy-tables`：SQL 使用主库词表 LIKE
  - `fts5-sidecar`：SQL 使用 ATTACH 后的 FTS5 MATCH
  - `unavailable`：第四个 OR 未加入（无词表、无可用 sidecar、或本 query 不能构造 MATCH）
- `library list` 与空 query 不输出该字段。
- `zot --json doctor` 在 `local_sqlite_read.available=true` 时附加 `fulltext.legacy_tables` 与 `fulltext.sidecar_present`。词表或 sidecar 缺失不得把 `available` 打成 `false`。doctor 只做文件系统与主库表探测，不 Backup 119 MB sidecar。
- human doctor 打印全文探测。`search-count` 只用于真正的 COUNT SQL 失败。

### R3 双 schema 回归与文档

- 保留 schema 120 富夹具：标题命中与附件词表命中（现有 `searches_titles_and_fulltext`）。
- 新增无词表、有 `fulltextItems` 的夹具：非空 query 必须 `Ok`，能命中标题/作者/标签。
- 新增无词表 + 迷你 `fulltext.sqlite` 夹具：query 命中附件 FTS 正文对应的父主条目，`meta.fulltext_index=fts5-sidecar`。
- `src/zot-local/tests/search_regression.rs` 覆盖上述路径。
- 更新 `.trellis/spec/zot-local/backend/database-guidelines.md`、`error-handling.md`，以及 `docs/agents/limits.md`、`docs/cli/library.md`、`skills/zot/SKILL.md`（经 `just skills-sync` 镜像）。

### R4 只读快照边界

- 不写入 `zotero.sqlite` 或源 `fulltext.sqlite`。
- 不恢复 `immutable=1`，不用 DB/WAL/SHM 手工复制。
- sidecar 必须用与主库相同的 Backup API 拷进 `LocalLibrary` 已有 `TempDir`，再 `ATTACH` 该临时文件。禁止 ATTACH live `data_dir/fulltext.sqlite`。
- sidecar Backup 在首次需要 FTS5 的非空 search 时惰性执行，不在 `open` / doctor / list 路径复制。
- sidecar 缺失、busy、integrity 失败时：主库仍可读，search 返回 metadata 命中，`fulltext_index=unavailable`。不得因此把 `LocalLibrary::open` 打成 `zotero-db-busy`。
- 不扩大 list / collection / tags / citekey / notes search 的 SQL 范围。

### R5 FTS5 附件全文匹配

- 仅在词表缺失且 sidecar 快照 ATTACH 成功时启用。
- FTS `rowid` 是附件 `itemID`。EXISTS 经 `itemAttachments.parentItemID = i.itemID` 映射到主条目，与旧词表 JOIN 同一语义。
- MATCH 字符串作为绑定参数。非 CJK：对 query 分词后使用 `"token1 token2"*`（末词前缀），至少一枚长度 ≥ 3 的 token 才加入 FTS 分支。纯 CJK：对 `fulltextContentCJK` 使用 2-gram 短语。CJK 与非 CJK 混写：省略 FTS 分支，metadata OR 仍执行。
- 不查询 `fulltextNotes*`。不扫描 `.zotero-ft-cache` 做多词短语二次校验。
- workspace `rusqlite` 保持现有 `bundled`（libsqlite3-sys 已 `-DSQLITE_ENABLE_FTS5`）。不为此新增 crate。

## Acceptance Criteria

- [x] AC1（R1）：无词表夹具上 `search(query="IEEE")` 返回 `Ok`，`total` 为 metadata 命中数；同夹具 `library list` 与改前一致。
- [x] AC2（R1/R3）：schema 120 富夹具上标题命中与附件词表命中与改前一致；`escape_like` 的 `%` / `_` / `\` 语义不变。
- [x] AC3（R2）：无词表且无 sidecar 的非空 search JSON `meta.fulltext_index=unavailable`；有词表夹具为 `legacy-tables`；空 query 与 `library list` 不出现该字段。
- [x] AC4（R5/R2）：无词表 + 迷你 sidecar 夹具上，仅存在于附件 FTS 正文的词能命中父主条目，`meta.fulltext_index=fts5-sidecar`。纯 metadata 词仍命中。
- [x] AC5（R2/R4）：doctor JSON 在 SQLite 可读时给出 `fulltext.legacy_tables` 与 `fulltext.sidecar_present`；二者为 false 时 `available` 仍为 true。夹具不再返回 `search-count` / `no such table: fulltextItemWords`。本机 live search 在 Zotero 占用库时为既有 `zotero-db-busy`。
- [x] AC6（R4）：测试证明 search 不 ATTACH live sidecar 路径；sidecar busy/缺失时 search 仍 `Ok` 且 `fulltext_index=unavailable`。
- [x] AC7（R3）：`cargo test -p zot-local`、相关 `zot-cli` doctor/search/envelope 测试与 `just ci` 通过；spec、limits、library 文档与 canonical skill 已同步。

## Out Of Scope

- 修改 Zotero 拥有的 schema，或强制 Zotero 重建全文索引。
- `library semantic-search`、workspace RAG、`item fulltext` PDF 抽取。
- 主条目搜索包含 note / annotation 子条目。
- 笔记 FTS 表 `fulltextNotes*` 与 `noteText` LIKE 回退。
- 对照 `.zotero-ft-cache` 做 Zotero 式多词短语二次校验；多词 MATCH 允许与 Zotero UI 在标点切分上存在候选级差异。
- 完整移植 `normalizeForSearch`（变音符号折叠等）。
- 跨命令缓存 sidecar 快照。
- GitHub Issue 创建或评论。

## Authority

规划待用户批准本轮最终摘要后，才允许 `task.py start` 与产品代码改动。创建任务与确认 Q1 不构成实施授权。
