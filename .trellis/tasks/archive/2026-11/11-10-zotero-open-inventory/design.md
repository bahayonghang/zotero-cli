# 打开状态下的分类与全库读取

## 边界

读路径复用 `desktop_read::ReadIo::route(false)` 的探针、绑定和 `--running-zotero`。这十条命令都要长出该参数。探测成功就使用桌面，并且不增加 `snapshot_open_attempts`。探针不可用、超时或 403 时，使用路由已经放入 `Route::Snapshot` 的那一份 `LocalLibrary`。不得再调用 `run_local`（`util.rs:23-37`）或 `ctx.local_library()`。

没有已证明的 Server ID 时，只有 `--running-zotero` 可以读当前实例，`read_source` 为 `desktop-unbound`。读请求走 `read_exchange(path, None)`，不发送 `Zotero-Server-ID`。有绑定 ID 时继续发送并核对。

写路径不动。`collection_snapshot` 仍要求 `server: &str`，版本变化仍立即返回 `local-api-conflict`。读路径不调用它，避免把无 ID 读取、显式页大小和 3 次版本重试塞进写入预览。

一条命令一个来源。普通 GET 不调用 `authorize`，不添加 `Zotero-API-Key`。

## 数据流

```text
十条读命令
  -> route(snapshot_first = false)
  -> Desktop：read_exchange，limit=100
  -> Snapshot：使用路由返回的 LocalLibrary
  -> 现有 JSON / 人类输出
```

桌面分支自己完成过滤、排序和切片。快照分支调用今天的 `get_tags`、`get_stats_with_trashed`、`search` 和分类函数。两边不共用一个新的统计口径。

## 分类树

分类列表使用新的只读分页，而不是 `collection_snapshot`。每页请求带 `limit=100` 和 `start`。组装规则仍对齐 `load_collections_tree`（`db.rs:974-1056`）。

10.0.8 的 `GET /collections` 不含回收站分类，而且忽略 `includeTrashed`。桌面列表跟 API 走。关闭应用时的 SQL 不改。桌面结果在 `meta` 里标明分类回收站被省略。

## 分类成员、计数和标签

成员集合先确定，再派生三个命令。

请求：

```text
GET {prefix}/collections/{key}/items/top?includeTrashed=1&limit=100
```

10.0.8 对不带 `/top` 的 `/items` 加 `includeChildren`，一篇入分类的文章加上它的子笔记会变成两项。`collectionItems` 只有直接成员。`/items/top` 加 `noChildren`，排除有父项的笔记、附件和注释。`includeTrashed=1` 才会带回回收站里的直接成员；SQL 的 `get_collection_items`（`db.rs:1106-1118`）本来就不过滤 `deletedItems`。

`item-count` 是这个集合的长度，不读不带 `/top` 的 `Total-Results`。`collection items` 是这个集合。`collection tags` 只统计这个集合上的标签，顺序与 `get_collection_tags`（`:1133-1144`）相同：计数降序，名称升序。

不请求 `/collections/{key}/tags`。10.0.8 没有注册它。`/collections/{key}/items/tags` 会走 `includeChildren`，子笔记的标签会被算进去。

分类搜索还会加 `itemType isNot annotation`。直接挂在分类上的注释不会出现在桌面结果里。快照 SQL 仍可能返回它。桌面 `meta` 标明分类注释被省略。实现不得为了补齐而打开 SQLite。

## 全库清单

`library list` 的书目过滤与 `db.rs:373-384` 相同：排除 `attachment`、`note`、`annotation`，默认排除回收站。条目上的 `collections` 继续用 `map_collection_keys`。

Zotero 在多条结果时拒绝 `sort=key`，默认按 `dateModified`。客户端不发送 `sort=key`。它用 `limit=100` 取完过滤后的全部页，在本地按 key 降序排列，再切 `offset/limit`。只排序当前页会在跨页时得到错误的 key 序列。

`--collection` 先解析再过滤。key 优先。名称必须唯一，否则返回 `collection-ambiguous`，候选 key 按现有 SQL 的 key 升序（`db.rs:1970-2015`）。解析使用同一来源里的分类列表，不额外打开快照。

## 库统计和库标签

`library stats` 的每个字段复现 `get_stats_with_trashed`（`db.rs:1758-1832`）：

| 字段 | 定义 |
| --- | --- |
| `total_items`、`by_type`、`collections` | 书目条目。默认排除回收站条目自身 |
| `top_tags` | 这些书目条目上的标签 |
| `pdf_attachments` | 不同的书目父项数。子项 MIME 为 `application/pdf`。两个 PDF 共用一个父项只算 1 |
| `notes` | 书目父项下的笔记行数。同一文章两条笔记算 2。没有书目父项的笔记算 0 |

`pdf_attachments` 和 `notes` 的回收站条件看父项。不能改成 `itemType=attachment` 或 `itemType=note` 的 `Total-Results`。

`library tags` 复现 `get_tags()`（`db.rs:706-716`）。它统计该库全部 `itemTags`，包含附件、笔记、注释和回收站。`rich_fixture_library` 上 `attention` 为 2。同一夹具的 `top_tags` 为 1。桌面实现可以从包含笔记和回收站的条目页汇总，但汇总结果必须等于 `get_tags()`，包含顺序。

## 分页和版本

完整读取的上限是独立常量：每页 100 条，最多 100 页。请求必须带 `limit=100`。省略 `limit` 时 10.0.8 会一次返回剩余的全部结果，不能用来履行 10_000 条上限。

恰好取满 10_000 条且没有下一页时成功。还存在第 10_001 条时返回未完成错误，不带 `total`，不返回半截列表。错误码在 `desktop-search-incomplete` 和专用新码之间选一个，规格和 `docs/agents/limits.md` 使用同一个名字。探索性 `library search` 的 `max_pages` 仍是 20。

版本或 `Total-Results` 在页与页之间变化时，整次读取作废并重试，最多 3 次，然后 `desktop-read-inconsistent`。不要使用 `collection_snapshot` 那种一次就 `local-api-conflict` 的行为。

## 兼容

- 关闭 Zotero 时不需要 `--running-zotero`。
- 已配置数据目录或 profile 时，自动桌面读取仍要求绑定过的 server id。不为了刷新绑定去撞锁住的库。
- `group:<id>` 使用 `prefix(scope)`。
- 对象级 404、412 和重定向不打开快照。
- `doctor` 的快照行可以继续报告忙。提示改为：快照被占用；桌面读取可用时，这十条命令不必关闭 Zotero。

## 取舍

完整结果先取齐再在本地排序，是因为 10.0.8 不能按 key 分页。10_000 条是这条命令的硬上限。超过上限直接失败，避免把默认的 `dateModified` 页冒充 key 降序页。

`library tags` 保持现有的全库口径。把它改成书目口径会让关闭 Zotero 时的输出也变化，超出本任务。

## 回滚

回滚十条命令的路由和新增的 `--running-zotero`。`collection_snapshot` 保持写入预览原样。规格、skill 和 `doctor` 提示与路由一起回滚。
