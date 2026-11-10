# collection 命令

`collection` 用于查看、搜索和维护真实的 Zotero collection。

## 子命令

```bash
zot --json collection list
zot --json collection get COLL001
zot --json collection subcollections COLL001
zot --json collection items COLL001
zot --json collection item-count COLL001
zot --json collection tags COLL001
zot --json collection search Transform --limit 20
zot --json collection create "New Project"
zot --json collection create "待读" --write-backend local
zot --json collection create "子类" --parent ROOT2345 --write-backend local
zot --json collection rename COLL001 "Renamed Project"
zot --json collection delete COLL001
zot --json collection delete COLL0001 --write-backend local
zot --json collection delete COLL0001 --write-backend local --confirm --expected-server-id <server_id> --expected-version <version> --expected-descendants-sha256 <sha256>
zot --json collection add-item COLL001 ATTN001
zot --json collection remove-item COLL001 ATTN001
```

## 什么时候使用 collection

`create`、`add-item` / `remove-item` 默认使用 Web API，也可显式指定 `--write-backend local`。本地创建的确认参数是 `--expected-library-version`，不是条目 version。详见[本地内容操作](./local-write.md)。
`delete` 不带 `--write-backend local` 时仍立即走 Web API。显式 local 时默认只预览。确认必须同时带预览中的 Server ID、分类对象 version 和子孙 key 集合 SHA-256。本地 key 必须是八位大写字母或数字。删除会永久擦除该分类和全部子分类，包括 Zotero 10.0.8 分类列表里看不到的废纸篓子分类；子分类不会提升到父级；文献留在库里，只失去这些成员关系。重命名和重挂父级仍只走 Web。
Zotero 10+ 的 `move-item ITEMKEY --from SOURCEKEY --to TARGETKEY --write-backend local`
会预览同库分类移动并保留其他分类。确认参数见[本地内容操作](./local-write.md)。

- 你在整理 Zotero 库本身的分组结构
- 你需要把条目挂到某个真实 collection 下
- 你想读某个 collection 的子 collection、条目数量或常见标签
- 你明确想修改远端 Zotero collection

## 与 workspace 的区别

- `collection`：修改 Zotero 里的真实 collection
- `workspace`：在本地维护一个阅读 / 检索工作区，不直接改 Zotero collection

如果你只是想围绕一个研究主题构建长期查询集合，优先用 [workspace](/cli/workspace)。

## 删除前提醒

`collection delete` 是破坏性动作。只有在用户明确要求删除时才执行。
