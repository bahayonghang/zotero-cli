# Zotero 10.0.8 本地删除分类的实际语义

证据来自本机已安装的 `C:\Program Files\Zotero\app\omni.ja`（10.0.8）。根目录那个 `omni.ja` 不含这些脚本。官方文档页和 GitHub raw 在本环境被拦截，不以它们作为契约来源。

## 路由

`chrome/content/zotero/xpcom/server/server_localAPI.js`：

- 单对象 `Collection` 的 `supportedMethods` 含 `GET`、`PUT`、`PATCH`、`DELETE`。路径是 `/api/(users|groups)/:id/collections/:collectionKey`。`DELETE` 调用 `deleteSingleObject(..., 'collection')`。
- 集合路径 `/api/(users|groups)/:id/collections` 另有批量 `DELETE`，上限 50 把 key，缺 key 会跳过。本任务不实现批量路径。
- `/collections/top` 和 `/collections/:collectionKey/collections` 拒绝 `POST`/`DELETE`（405）。
- 写请求要求 `Zotero-API-Key` 和 `Zotero-Server-ID`。这是应用自己的对象 API，不是写 `zotero.sqlite`。

## 擦除时文献还在

`chrome/content/zotero/xpcom/data/dataObject.js` 的 `erase` 把 `options.deleteItems` 记为 “Move descendant items to trash (Collection only)”。`eraseTx` 只补上 `options.tx = true`，再调用 `erase`。Local API 的 `deleteSingleObject` 调用 `eraseTx()` 时不传 options，所以 `deleteItems` 为假。

`chrome/content/zotero/xpcom/data/collection.js`：

- `trash` 把目标分类和子孙分类放进删除集。只有 `env.options.deleteItems` 时才会动子孙文献。
- `_eraseData` 若对象尚未删除就先 `trash`，然后永久删除这些分类的 `collectionItems` 行、清空父指针，并从 `collections` 表删除目标和子孙分类。文献行保留，只是内存里的分类缓存丢掉被擦掉的分类 id。

因此一次单对象 DELETE 的效果是：

- 目标分类和全部子孙分类被永久删除，不是把子分类提升到父级。
- 这些分类上的成员关系被去掉。
- 文献不被扔进废纸篓，也不被删除；它们在其他分类里的成员关系保留。

父对象的 `clientVersion` 不会因为新增子分类而必然变化。确认条件不能只绑父版本。

## 成功与失败

单对象 DELETE 的成功是 HTTP 204，并且响应的 `Zotero-Server-ID` 与请求实例一致。随后对同一 key 的 GET 返回 404，才表示回读确认对象已不在。

- DELETE 本身返回 404：请求时对象已经不在。不能把这次调用说成删除成功。
- HTTP 200、超时、5xx、Server ID 不符：结果未知。不得因为之后某次 GET 404 就改口说这次删除成功。
- `If-Unmodified-Since-Version` 缺失是 428，非法是 400，对象 `clientVersion` 大于期望值是 412。比较使用的是分类对象版本，不是库版本。版本 0 是合法值；本库列表里出现过 0，且条件是 `clientVersion > expected`。
- 预览里没有版本时，不得自行填 0。

## 列表看不到废纸篓分类

`server_localAPI.js` 的集合 GET 调用 `Zotero.Collections.getByLibrary(libraryID, !top)`，没有第三个参数。`collections.js` 的 `getByLibrary(libraryID, recursive, includeTrashed)` 默认 `includeTrashed` 为 false，因此已在废纸篓中的分类不会进入列表。同文件里的 `includeTrashed=1` 只加在条目搜索上，分类列表不读这个参数。

单对象 DELETE 仍会永久擦除目标及其子孙分类，包括列表里没有的废纸篓子分类。预览哈希只能冻结列表返回的 key。不要打开 SQLite 去补这张列表，也不要把哈希说成已经覆盖废纸篓。

## 对 CLI 的直接后果

`zot collection delete` 原先只调用 Web API。本地路径做单 key 预览/确认，并把列表中的子孙 key 集合冻结进确认哈希。不要用 SQLite，不要在失败后改走 Web。
