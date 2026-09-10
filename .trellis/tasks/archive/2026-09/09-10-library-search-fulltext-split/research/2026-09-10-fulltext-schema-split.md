# 2026-09-10 library search 与 Zotero 全文索引拆分

## 本机复现

Invocation：已安装的 `zot`（`C:\Users\lyh\.cargo\bin\zot.exe`）。

| 命令 | 结果 |
| --- | --- |
| `zot --json doctor` | `ok=true`；`capabilities.local_sqlite_read.available=true`；`schema_version=129`；`data_dir=C:\Users\lyh\Zotero` |
| `zot --json library search IEEE --limit 3` | `ok=false`；`error.code=search-count`；`error.message=no such table: fulltextItemWords` |
| `zot --json library list --limit 1` | `ok=true`；`meta.total=1765` |
| `zot --json library search "" --limit 1` | `ok=true`（空 query 不进入词表 JOIN） |

## 本机 schema

`C:\Users\lyh\Zotero\zotero.sqlite`：

- `version.userdata = 129`
- `version.fulltext_1 = 25862`
- `sqlite_master` 含 `fulltextItems`，不含 `fulltextItemWords`、`fulltextWords`

`C:\Users\lyh\Zotero\fulltext.sqlite` 约 119 MB。FTS5 虚表：

- `fulltextContent` / `fulltextContentCJK`（附件正文）
- `fulltextNotes` / `fulltextNotesCJK`（笔记正文）
- `fulltextIndexState(itemID, version)` 约 2064 行
- `fulltextContent` 约 1968 行

## 上游迁移

Zotero `chrome/content/zotero/xpcom/schema.js` userdata **127**：

```
DROP TABLE IF EXISTS fulltextItemWords;
DROP TABLE IF EXISTS fulltextWords;
```

`resource/schema/userdata.sql` 头注释 `-- 129`。`CREATE TABLE` 列表含 `fulltextItems`，不含词表。

`chrome/content/zotero/xpcom/fulltext.js`：全文内容索引放在 ATTACH 的 `fulltext.sqlite`，contentless FTS5。主库 `fulltextItems` 保留为索引元数据。

## 当前 zot 路径

- 非空 query JOIN：`src/zot-local/src/db.rs:226-235`
- 失败点：`search-count` `src/zot-local/src/db.rs:281-286`
- 快照只复制 `zotero.sqlite`：`src/zot-local/src/db.rs:1622-1661`
- 可选表先例：`table_exists` + `itemAnnotations` → 空列表，`src/zot-local/src/db.rs:558-560`、`:2190-2199`
- 夹具仍为 userdata 120 并创建词表：`src/zot-local/src/db.rs:2985-2986`、`:3005-3006`

## FTS5 查询合同（Zotero `fulltext.js`）

- 虚表：`USING fts5(text, tokenize='unicode61', content='', contentless_delete=1)`；CJK 表 `tokenize='ascii'`。
- `rowid` = 附件 `itemID`。`SELECT C.rowid FROM ftindex.fulltextContent C JOIN items I ON I.itemID=C.rowid WHERE C.fulltextContent MATCH ?`。
- 非 CJK MATCH：`"token1 token2"*`（末词前缀）。纯 CJK：2-gram 短语打到 `fulltextContentCJK`。混写返回 null。
- 单 token 长度 < 3 时 `canSearchContent` 为 false。多词短语在 Zotero 内还要扫 `.zotero-ft-cache`；本任务不做该校验。
- workspace rusqlite `bundled` 对应 libsqlite3-sys 0.37，sqlite 3.51.3，编译带 `-DSQLITE_ENABLE_FTS5`。无需新 crate feature。

## 对设计的约束

1. 缺词表时必须在 SQL 拼装阶段省略 JOIN，不能靠捕获 `no such table`。
2. sidecar 必须 Backup 进现有 `TempDir` 再 ATTACH `ftindex`，不能 ATTACH live `data_dir/fulltext.sqlite`。
3. 全文语义保持「附件 rowid 命中 → parentItemID 主条目」。不查 `fulltextNotes*`。
4. 2026-09-10 用户确认 Q1：本任务接 FTS5 sidecar。
5. sidecar busy/缺失时 metadata search 仍须 `Ok`，`fulltext_index=unavailable`。
