# Design: library search 与 Zotero 7.1+ 全文拆分

## Architecture and boundary

`LocalLibrary::open` 仍只 Backup `zotero.sqlite`。全文 sidecar 不进入 open / doctor / list。

非空 `search` 在拼 WHERE 之前选择至多一个全文后端：

```text
if fulltextItemWords && fulltextWords exist
    -> legacy LIKE branch, meta=legacy-tables
else if MATCH clause is constructible
    -> lazy Backup data_dir/fulltext.sqlite into existing TempDir
    -> ATTACH snapshot AS ftindex (never the live path)
    -> FTS5 EXISTS on ftindex.fulltextContent or fulltextContentCJK
    -> meta=fts5-sidecar
else
    -> omit fourth OR, meta=unavailable
    -> metadata OR still runs
```

`LocalLibrary` 已有 `data_dir` 与 `OnceCell`（`src/zot-local/src/db.rs:133-141`）。用第二枚 `OnceCell` 记住本次连接的 sidecar ATTACH 结果，避免同一 `search` 的 COUNT 与 page 查询重复 Backup。

## Data flow

```text
query
  -> metadata EXISTS (itemData / creators / tags) always
  -> exclusive fulltext backend (legacy XOR fts5 XOR none)
  -> COUNT(*) + page IDs + get_items_batch
  -> SearchResult { items, total, query, fulltext_index }
  -> zot-cli EnvelopeMetaSeed.fulltext_index
  -> EnvelopeMeta.fulltext_index (skip_serializing_if None)
```

`library search` 的 JSON `data` 仍是 `items` 数组。后端状态只走 `meta`，与 `trash_policy` 相同。

## Contracts

### Legacy tables

复用 `table_exists`。两表都在才拼 `db.rs:232` 的 LIKE EXISTS。缺一即视为无词表，改走 sidecar 或省略。

### Sidecar snapshot

- 源：`data_dir.join("fulltext.sqlite")`，`SQLITE_OPEN_READ_ONLY` + 现有 5s busy timeout。
- 目标：`TempDir/fulltext.sqlite`，`run_snapshot_backup` 与主库同一 `SnapshotPolicy`。
- 完成后关闭可写目标，`ATTACH` 临时文件为 `ftindex`。禁止 URI `immutable=1`，禁止 ATTACH 源路径。
- `PRAGMA quick_check` 非 `ok`、busy/locked、源缺失：不抛给调用方；本 search 的全文后端为 `unavailable`。
- 主库 `open` 的 busy 仍映射 `zotero-db-busy`。sidecar 失败不得冒充主库锁失败。

### FTS5 MATCH

Zotero `fulltext.js`：`SELECT C.rowid FROM ftindex.fulltextContent C JOIN items I ON I.itemID=C.rowid WHERE C.fulltextContent MATCH ?`。contentless 表的 `rowid` 即附件 `itemID`。

zot 的 EXISTS（主条目 i）：

```sql
EXISTS (
  SELECT 1 FROM itemAttachments ia
  JOIN ftindex.fulltextContent fc ON fc.rowid = ia.itemID
  WHERE ia.parentItemID = i.itemID
    AND fc.fulltextContent MATCH ?
)
```

纯 CJK 把表换成 `fulltextContentCJK`。MATCH 文本绑定，不拼接进 SQL。

构造规则（缩小自 `getWordMatchClause`，不做 cache verify）：

| query | 行为 |
| --- | --- |
| 无可用 token | 省略 FTS |
| 非 CJK，最长 token < 3 | 省略 FTS（与 `canSearchContent` 一致） |
| 非 CJK，否则 | `"token1 token2"*`，`"` → `""` |
| 纯 CJK，长度 ≥ 2 | 相邻 2-gram 短语打到 CJK 表 |
| 单字 CJK 或 CJK+拉丁混写 | 省略 FTS |

workspace rusqlite `bundled` 已编译 `SQLITE_ENABLE_FTS5`（libsqlite3-sys 0.37 / sqlite 3.51.3）。不新增 feature。

### Observability

`EnvelopeMeta` / `EnvelopeMetaSeed` 增加 `fulltext_index: Option<String>`。取值：`legacy-tables` | `fts5-sidecar` | `unavailable`。`api_version` 保持 1。

`SearchResult` 增加同名可选字段，供 CLI 填 seed。list / 空 query 保持 `None`。

doctor：`capabilities.local_sqlite_read.fulltext = { legacy_tables, sidecar_present }`。`sidecar_present` 只检查 `data_dir/fulltext.sqlite` 存在。

## Error matrix

| Condition | Result |
| --- | --- |
| 主库词表缺失 | 省略 LIKE 分支，不报 `search-count` |
| sidecar 文件缺失 | `fulltext_index=unavailable`，metadata search `Ok` |
| sidecar Backup busy/locked | 同上，不返回 `zotero-db-busy` |
| sidecar quick_check 失败 | 同上 |
| MATCH 不可构造 | 省略 FTS 分支，`unavailable` |
| 真正 COUNT SQL 失败 | 仍为 `search-count` |

## Compatibility and trade-offs

- schema ≤126：行为与现 LIKE 全文一致。
- schema ≥127 无 sidecar：关键字 search 恢复为 metadata-only，不再硬失败。
- schema ≥127 有 sidecar：附件全文用 FTS5 前缀/短语，不再用词表 `LIKE %q%`。单字符、混写脚本、多词标点短语与 Zotero UI 可能不一致；PRD 已接受不做 cache verify。
- 惰性 Backup 把约 119 MB I/O 留在非空 FTS search，避免 doctor/list 每次复制。每个 `run_local` 进程仍付一次。
- 不 ATTACH live sidecar：避免 WAL 下读到半提交 FTS 页，并遵守 `07-26-fix-sqlite-snapshot` 合同。

## Rollback

整任务提交作为一单元回退。部分回退不得留下 ATTACH live 路径，也不得把词表 JOIN 无探测地恢复到 schema 129。
