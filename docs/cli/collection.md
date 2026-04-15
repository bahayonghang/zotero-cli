# collection 命令

`collection` 用于查看、搜索和维护真实的 Zotero collection。

## 子命令

```bash
zot --json collection list
zot --json collection items COLL001
zot --json collection search Transform --limit 20
zot --json collection create "New Project"
zot --json collection rename COLL001 "Renamed Project"
zot --json collection delete COLL001
zot --json collection add-item COLL001 ATTN001
zot --json collection remove-item COLL001 ATTN001
```

## 什么时候使用 collection

- 你在整理 Zotero 库本身的分组结构
- 你需要把条目挂到某个真实 collection 下
- 你明确想修改远端 Zotero collection

## 与 workspace 的区别

- `collection`：修改 Zotero 里的真实 collection
- `workspace`：在本地维护一个阅读 / 检索工作区，不直接改 Zotero collection

如果你只是想围绕一个研究主题构建长期查询集合，优先用 [workspace](/cli/workspace)。

## 删除前提醒

`collection delete` 是破坏性动作。只有在用户明确要求删除时才执行。
