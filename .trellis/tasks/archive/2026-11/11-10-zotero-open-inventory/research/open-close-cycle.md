# 为什么整理本机库要反复开关 Zotero

日期：2026-11-10。本文件只记录证据和被否决的做法。实现契约在 `design.md`。

## 本会话里的开关

分类整理任务 `.trellis/tasks/archive/2026-11/11-09-zotero-collection-taxonomy/prd.md` 第 9–11 行记下了关着 Zotero 时的导出。命令是 `zot --json doctor`、`collection list`、`library stats`、`library tags`、`library list --limit 2500`、`library search --title`。当时 Local API 不可达。快照给出 175 个分类、1793 条非附件非笔记顶层条目。侧边栏 My Library 是 1797，差 4 条，原因没有再拆。

会话里随后出现两轮人工切换：先「已关闭 Zotero，继续导出」，再「已打开 Zotero，继续移动预览」。关应用是为了 SQLite 快照。开应用是为了 Local API 写入（创建分类、移动条目、后来的删除）。

同一天删空分类时 Zotero 开着。`zotero-db-busy` 仍出现在快照上。分类是否为空改走 Local API：`GET /collections`，以及 `GET /collections/KEY/items` 和 `/items/top` 的 `Total-Results`。当时 Server ID 是 `2ZxitCcOtvOu`。记录在已归档任务的 `research/empty-collections.md`。这个 ID 是当时实例的值，不是要写进代码的常量。

## 当前代码把哪些命令钉在快照上

这些读命令在探测成功时仍会打开快照，所以 Zotero 开着就会 `zotero-db-busy`：

| 命令 | 位置 | 行为 |
| --- | --- | --- |
| `collection list/get/subcollections/search/items/item-count/tags` | `src/zot-cli/src/commands/collection.rs:106` | 读分支直接 `local_library()` |
| `library list` | `src/zot-cli/src/commands/library.rs:82-102` | `run_local` → `library.search`，不走桌面路由 |
| `library stats` | 同文件 `:119-132` | 同上 |
| `library tags` | 同文件 `:138-145` | `local_library()` |

已经能在 Zotero 开着时读取的命令不要重做：

- `item get`、`item children`、`item pdf`、`library citekey`，以及空查询的 `library search`（含 `--title`）。路由在 `src/zot-cli/src/commands/desktop_read.rs:175`：只有非空通用查询才先开快照。
- 非空 `library search` 仍先付 5 秒快照忙等，失败后才可能落到桌面，并且全文索引记为不可用。这不是这次整理来回开关的原因。
- 本地写入已经要求 Zotero 开着。本任务不改写路径。

快照忙等是 5 秒：`src/zot-local/src/db.rs:31` 的 `ZOTERO_DB_BUSY_TIMEOUT`。失败提示现在写「Close Zotero…」：同文件 `:3061-3066`。

桌面检索默认 100 行一页，最多 20 页：`src/zot-desktop/src/local_read.rs:76-82` 和 `:522`。20 页是 2000 行。1793 条刚好低于这个数，再长就会 `desktop-search-incomplete`。`library list --limit 2500` 不能复用这个上限。

`LocalApiClient::request`（`local_api.rs:146-164`）只加 `Zotero-API-Version: 3` 和可选的 `Zotero-Server-ID`。`collection_snapshot`（`:420-533`）只服务写入预览：它要求 `server: &str`，请求不带 `limit=100`，版本变化立即是 `local-api-conflict`。读命令不能把它当成 10_000 条上限的分页器。`read_exchange`（`:205`）接受可选 Server ID。

`.trellis/spec/zot-cli/backend/desktop-read.md` 第 5–14 行的范围是条目、PDF、检索和 citekey。分类命令和 `library list/stats/tags` 不在其中。

## SQLite 在应用打开时为什么读不到

已归档任务 `11-04-zotero-open-library-reads` 的 `design.md` 核对过安装包：`db.js` 把主库设成连接级独占锁（`locking_mode=EXCLUSIVE`）。只读连接在对方未关闭时得到 `SQLITE_BUSY`。仓库测试 `held_exclusive_connection_blocks_snapshot_until_dropped` 锁住同一行为。

公开资料和这个结论一致：

- Dan Stillman 在 [zotero/zotero#4860](https://github.com/zotero/zotero/issues/4860) 写道：Zotero 故意使用独占锁，因为它缓存大量数据，外部写入在打开期间不会被看见。`openNotExclusive` 当时未作为默认。`ignoreLockingMode` 意味着只读，只被当作网络共享的可能办法讨论过。
- 更早的论坛帖里，Stillman 写明 Zotero 不支持多个客户端同时访问本地数据库（[forums.zotero.org/discussion/19861](https://forums.zotero.org/discussion/19861/accessing-the-zotero-sqlite-db-with-multiple-clients)）。
- 官方论坛对「开着应用用 Python 读 sqlite」的回复是：数据库正在被使用，这样读一般不安全；有人改拷一份临时库，并指出大库会很慢（[discussion/comment/305187](https://forums.zotero.org/discussion/comment/305187/)）。
- [Zotero data directory](https://www.zotero.org/support/zotero_data) 说明 `zotero.sqlite-wal` 和 `-shm` 在应用打开时存在。WAL 通常在干净退出时被合并。复制时必须带上 WAL，不能单独删掉。
- WAL 模式本身也会让短命读者遇到 `SQLITE_BUSY`（[hynek.me/til/sqlite-read-only-wal-locked](https://hynek.me/til/sqlite-read-only-wal-locked/)）。把忙等从 5 秒再加长，解不开独占锁。

## 开着应用时可用的读接口

Zotero 7 beta 88 起提供本机 HTTP API，声明为 Web API 的近乎完整只读实现，读本地数据不必再直接打开 SQLite（[zotero-dev 公告](https://groups.google.com/g/zotero-dev/c/ElvHhIFAXrY)）。条件是设置里允许本机其他应用与 Zotero 通信。示例是 `GET http://localhost:23119/api/users/0/items`。

第三方 [dsh-zotero-local](https://www.npmjs.com/package/dsh-zotero-local)（2026-11 发布说明）把同一边界写成：`127.0.0.1:23119` 上的读取不需要密钥；写入才要一次性本地授权。它列出分类和计数、条目、全文和附件路径，并写明在 Zotero 9 和 10.0.7 上测过。

[zotlib](https://pypi.org/project/zotlib/) 直接读 `zotero.sqlite`。它没有去掉应用打开时的锁。

本仓库已经遵守同一边界：普通 GET 不调用 `authorize`，不发送写密钥。见 `desktop-read.md` 第 60–64 行。

## 被否决的做法

| 做法 | 为什么不作为主路径 |
| --- | --- |
| 加长 `busy_timeout` | 独占锁不会因为等多几秒就放开。整理流程等的是人去关应用。 |
| 开着应用拷贝 `zotero.sqlite` | 官方讨论认为不安全。漏掉 WAL 或赶上写入会得到半份库。大库也慢。 |
| `openNotExclusive` / `immutable` | 要改 Zotero 自己的打开方式，或把一份仍在写入的库当成不变文件。Stillman 拒绝把非独占锁当作默认。 |
| 把 20 页检索上限直接放大 | 探索性 `library search` 的全文和未完成语义会一起变。全库清单需要单独的完整读取。 |
| 让一条命令混用快照和 Local API | 现有契约是一条命令一个来源。混用会把两次提交拼成一份假快照。 |

## 审阅后核对过的口径

2026-11-10 的规划审阅要求桌面和快照对同一命令给出相同计数。下面几条已对照仓库和本机安装包 `C:\Program Files\Zotero\app\omni.ja` 里的 `server_localAPI.js`、`search.js`。没有访问用户的真实库。

`library tags` 与 `library stats.top_tags` 不是同一个统计。`get_tags()`（`db.rs:706-716`）没有类型过滤，也没有回收站过滤。夹具把 `attention` 同时挂在 `ATTN001` 和笔记 `NOTE004` 上（`:3535-3543`）。现有测试断言前一条命令的计数是 2（`:4066-4068`）。`top_tags` 的 SQL（`:1780-1788`）排除笔记，所以只剩 `ATTN001`，计数是 1。

分类成员不能用 `GET /collections/{key}/items` 的 `Total-Results`。10.0.8 在非 `/top` 路径加 `includeChildren`，会并入父项的子笔记和子附件。`/items/top` 加 `noChildren`，SQL 排除有 `parentItemID` 的笔记、附件和注释。`includeTrashed=1` 在条目搜索上会加入 `includeDeleted`。分类列表的 `includeTrashed` 仍然无效。

分类标签没有 `/collections/{key}/tags`。注册路径是 `/collections/:collectionKey/items/tags` 和带 `/top` 的同一路径。不带 `/top` 时，标签统计使用包含子项的条目集合。

分页必须自己带 `limit`。未传 `limit` 时，页大小等于剩余结果的全长。多条结果的默认排序是 `dateModified`，`sort=key` 返回 400。

`pdf_attachments` 和 `notes` 的精确定义在 `db.rs:1804-1823`。前者按书目父项去重，后者按笔记行计数，回收站都看父项。

仍不在验收夹具里的边界：

- 分类搜索另外排除 `annotation`。直接入分类的注释在桌面结果中缺失，快照 SQL 仍可能返回。
- 子项自己占一行 `collectionItems` 时，`/items/top` 会丢掉它。AC5 不构造这种行。
