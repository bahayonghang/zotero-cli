# library 命令

`library` 负责本地只读浏览与检索，是默认的“先查再读”入口。

## 子命令

- `library search <query>`
- `library list`
- `library recent <YYYY-MM-DD>`
- `library stats`
- `library duplicates`

## search

常用示例：

```bash
zot --json library search "transformer attention" --limit 10
zot --json library search "reward hacking" --collection COLL001 --limit 20
zot --json library search "alignment" --type journalArticle --sort date-added --direction desc
```

可用参数：

- `--collection <key>`
- `--type <item-type>`
- `--sort <date-added|date-modified|title|creator>`
- `--direction <asc|desc>`
- `--limit`
- `--offset`

## list / recent / stats / duplicates

```bash
zot --json library list --limit 20
zot --json library recent 2026-01-01 --limit 20
zot --json library stats
zot --json library duplicates --limit 20
```

## 推荐配合方式

典型顺序是：

1. `library search`
2. `item get`
3. `item cite` / `item export` / `item pdf`

如果你不是在处理单篇，而是在围绕一批论文构建长期主题集合，转到 [workspace](/cli/workspace)。
