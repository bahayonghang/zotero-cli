# Zotero 保持打开时读取分类和全库

## Goal

整理本机 Zotero 库时，分类树、分类成员、全库清单、库统计和库标签都能在 Zotero 打开的情况下读完。同一条命令在桌面读取和快照读取下给出相同的计数值和同一份结果。用户不必为了导出再关掉应用，也不必为了接着写入再打开。关着应用时，这些命令仍只打开一次现有快照。

## Confirmed facts

2026-11-10 的分类整理必须关着 Zotero 才能跑 `collection list`、`library stats`、`library tags` 和 `library list --limit 2500`。随后必须打开 Zotero 才能做 Local API 写入。证据在 [research/open-close-cycle.md](research/open-close-cycle.md)。

Zotero 对 `zotero.sqlite` 使用连接级独占锁。快照在 5 秒后返回 `zotero-db-busy`。加长忙等、拷贝正在使用的库、或改成非独占锁，都不能作为这条读取路径。

`item get`、PDF、citekey，以及带 `--title` 的空查询检索，已经可以在开着应用时读取。非空通用 `library search` 仍先开快照。本任务不改那条检索契约，也不改本地写入。

这十条读命令今天都没有 `--running-zotero`。`LibraryListArgs` 从 `src/zot-cli/src/cli/args.rs:266` 起只有 collection、limit、offset 和 include-trashed。分类读命令的参数结构同样没有这个开关。已有桌面读取用它表示未绑定的当前实例。

`route(false)` 在探针失败时已经打开一次 `LocalLibrary` 并放进 `Route::Snapshot`（`desktop_read.rs:333-339`）。`run_local` 会再调用 `LocalLibrary::open`（`util.rs:29`）。第二次打开会记入 `snapshot_open_attempts()`（`db.rs:42-56`）。

`collection_snapshot`（`local_api.rs:420`）要求 `server: &str`，请求路径不带 `limit=100`，版本一变就返回 `local-api-conflict`（同文件 `:461-467`）。它继续只服务写入预览。`read_exchange`（`:205-238`）已经接受 `Option<&str>`，没有 Server ID 时不做实例比对。

本机 Zotero 10.0.8 的 `server_localAPI.js` 还有这些读取行为：

- 未传 `limit` 时，页大小是剩余结果的全长（约第 396 行）。循环 100 次不等于最多 10_000 条。
- 结果多于 1 条时，默认排序是 `dateModified`。`sort=key` 不在允许列表里，会返回 400（约第 343–347 行）。
- 分类下的 `/items` 会加 `includeChildren`。子笔记和子附件被并进结果。`/items/top` 加 `noChildren`，排除有父项的笔记、附件和注释（`search.js` 约第 1281–1287 行）。
- 分类条目搜索另外加上 `itemType isNot annotation`。
- `includeTrashed=1` 在条目搜索上会加入 `includeDeleted`。分类列表本身仍忽略该参数。
- 已注册的分类标签路径是 `/collections/:collectionKey/items/tags` 和 `/items/top/tags`。没有 `/collections/:collectionKey/tags`。不带 `/top` 的标签路径同样会 `includeChildren`。

仓库夹具 `rich_fixture_library` 里，标签 `attention` 挂在书目条目 `ATTN001` 和笔记 `NOTE004` 上（`db.rs:3482-3543`）。`get_tags()`（`:706-716`）统计库内全部 `itemTags`，现有测试断言 `attention` 的计数是 2（`:4066-4068`）。`library stats` 的 `top_tags`（`:1780-1788`）排除附件、笔记和注释，所以这条笔记不计入，计数是 1。两条命令的口径不同。按同一种书目过滤实现 `library tags`，开关 Zotero 会改变它的输出。

`pdf_attachments`（`:1804-1813`）是 `COUNT(DISTINCT parentItemID)`，父项必须是书目类型，MIME 为 `application/pdf`。同一文章两个 PDF 只算 1。`notes`（`:1815-1823`）统计挂在书目父项下的笔记行，独立笔记没有书目父项，不计入。这两个字段的回收站过滤看父项，不看子项自己。

## Requirements

- R1 探测成功时，`collection list`、`collection get`、`collection subcollections`、`collection search`、`collection items`、`collection item-count`、`collection tags`、`library list`、`library stats`、`library tags` 只走这一次桌面读取。`snapshot_open_attempts` 不增加。零次 `authorize`，请求不带写密钥。`meta.read_source` 为 `desktop` 或 `desktop-unbound`。人类输出不另起 `Source:` 行。这十条命令都接受 `--running-zotero`，语义与现有条目读取相同。
- R2 探针失败或 403 时，上述命令消费 `Route::Snapshot` 里已经打开的那一份库。不得再调用 `run_local` 或 `local_library()`。`snapshot_open_attempts` 只增加 1。`meta.read_source` 为 `snapshot`。关着应用时，每条命令保持自己今天的 SQL 口径。
- R3 一条命令只用一个来源。已保存的 server id 不一致且没有 `--running-zotero` 时返回 `desktop-library-mismatch`。`--running-zotero` 允许没有 Server ID 的 `desktop-unbound`，读请求不强制发送 `Zotero-Server-ID`。对象 404、412 和重定向不改去另一个来源。写入预览仍走 `collection_snapshot`，仍要求 Server ID；读路径不调用它。
- R4 三个全库命令的口径分开，并且桌面与快照对同一命令逐值相同。
  - `library list` 默认排除附件、笔记、注释和回收站。`--include-trashed` 与 `meta.trash_policy` 保持原样。先取完整过滤结果，再按条目 key 降序排列，然后应用 `offset/limit`。不向 Zotero 发送 `sort=key`。`--collection` 接受 key 或唯一名称；重名返回 `collection-ambiguous`。
  - `library stats.top_tags`、`total_items`、`by_type` 和 `collections` 只统计书目条目。`pdf_attachments` 是拥有 `application/pdf` 的不同书目父项数。`notes` 是挂在书目父项下的笔记数。后两个字段的回收站过滤看父项。
  - `library tags` 沿用 `get_tags()`：统计该库全部条目的标签，包含附件、笔记、注释和回收站。它不使用书目过滤。
- R5 `collection items`、`collection item-count` 和 `collection tags` 使用同一组直接成员。直接成员是 `collectionItems` 里的条目。请求使用 `/collections/{key}/items/top?includeTrashed=1&limit=100`。不使用不带 `/top` 的 `/items` 总数，因为 10.0.8 会并入子笔记和子附件。不请求未注册的 `/collections/{key}/tags`，也不采用会并入子项的 `/items/tags`。列表、计数和标签都从这组条目生成。
- R6 完整读取显式发送 `limit=100`，并带 `start`。最多 100 页，即 10_000 条。恰好 10_000 条成功。第 10_001 条返回未完成错误，响应里没有 `total`，也没有半截列表。同一次读取记录 `Last-Modified-Version`。版本或 `Total-Results` 变化时最多重试 3 次，然后返回 `desktop-read-inconsistent`，而不是 `local-api-conflict`。探索性 `library search` 保持 20 页上限。
- R7 `skills/zot` 写明：这些命令在 Zotero 打开且本机通信可用时可以直接跑。`doctor` 的快照行仍可报告 `zotero-db-busy`，但不得把「先关闭 Zotero」写成这些命令的下一步。
- R8 个人库和 `group:<id>` 使用同一路由。模拟响应按 10.0.8 的实际形状构造：不带 `/top` 时包含子笔记，多条结果拒绝 `sort=key`，省略 `limit` 时一次返回全部。与快照对照的期望值来自同一份 SQLite 夹具上的现有函数。测试不连接本机正在运行的 Zotero，也不写用户的 `zotero.sqlite`。

## Acceptance Criteria

- [ ] AC1（R1、R3）：模拟独占锁期间，十条命令成功。`read_source` 为桌面来源，`snapshot_open_attempts` 的增量为 0，授权次数为 0，请求头没有写密钥。
- [ ] AC2（R2）：探针失败时每条命令的 `snapshot_open_attempts` 增量是 1。测试直接断言这个计数。实现若在拿到 `Route::Snapshot` 后再进 `run_local`，增量变为 2，测试失败。关闭状态下 `collection list` 仍返回 SQLite 今天会返回的回收站分类。
- [ ] AC3（R3）：server id 不一致且没有 `--running-zotero` 时失败，并且不读取另一侧。带 `--running-zotero`、响应没有可绑定的 Server ID 时，命令成功，`read_source` 为 `desktop-unbound`，请求不带 `Zotero-Server-ID`。分类 404 返回 `collection-not-found`，不回退快照。写入预览在缺少 Server ID 时仍失败。
- [ ] AC4（R4、R6）：客户端发出的完整读取带 `limit=100`。过滤后恰好 10_000 条时成功。10_001 条时失败，响应没有 `total`。带全文查询的 `library search` 仍在 20 页停止。
- [ ] AC5（R5）：夹具含一篇直接入分类的文章、它的一条子笔记，以及一个没有父项、直接挂在同一分类上的附件。成员 key 恰好是文章和附件，`item-count` 为 2。子笔记不在列表里。子笔记独有的标签不出现在 `collection tags`。快照侧的 `get_collection_items`、`get_collection_item_count` 和 `get_collection_tags` 给出同一组结果。
- [ ] AC6（R4）：在 `rich_fixture_library` 上，桌面和快照的 `library tags` 都报告 `attention` 为 2。两边的 `library stats.top_tags` 都报告 `attention` 为 1。`library tags` 的顺序与 `get_tags()` 相同：计数降序，名称升序。
- [ ] AC7（R4）：同一夹具上逐字段比较两端的 `library stats`。其中一篇书目文章有两个 `application/pdf`，`pdf_attachments` 只加 1。这篇文下有两条笔记，`notes` 加 2。一条没有书目父项的笔记不计入。父项在回收站时，默认结果不含它的 PDF 和笔记。
- [ ] AC8（R4）：`library list` 的夹具里，key 的降序与 `dateModified` 的顺序相反，并且结果跨页。默认输出的 key 序列等于完整过滤结果按 key 降序后再切 `offset/limit`。另覆盖分类 key、唯一名称，以及重名时的 `collection-ambiguous`。
- [ ] AC9（R6）：翻页中途库版本变化，第 3 次后得到 `desktop-read-inconsistent`，退出码为失败。写入预览的版本冲突仍是 `local-api-conflict`。
- [ ] AC10（R7、R8）：skill eval 覆盖「快照 busy、桌面可读」。`just ci` 通过。测试不连接正在运行的 Zotero。

## Out of Scope

- 非空通用 `library search` 的快照优先和 5 秒忙等。
- `library recent`、feeds、duplicates、semantic search、saved search。
- 本地写入、分类重命名、改父级、批量删除，以及 `collection_snapshot` 的身份校验。
- 修改 Zotero、加长 SQLite 忙等、拷贝打开中的数据库。
- 为了迁就 API 而改变 `get_tags()` 或 `get_stats_with_trashed()` 的 SQL。
- 把侧边栏 1797 和快照 1793 的差强行抹平。
- 10.0.8 分类搜索会丢掉的直接注释。桌面分类成员不含这类注释；快照 SQL 仍可能含有。本任务不打开 SQLite 把它们补进桌面结果，也不改关闭状态下的 SQL。
- 子项自己占一行 `collectionItems` 的库。AC5 不使用这种行。
- 本任务执行期间改用户的真实分类或条目。
