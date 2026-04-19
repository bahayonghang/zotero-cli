# library 命令

`library` 是默认的本地只读入口，负责“先查、再定位、再转到 item/workspace”这一层工作。

## 子命令

- `library search <query>`
- `library list`
- `library recent <YYYY-MM-DD>`
- `library stats`
- `library citekey <citekey>`
- `library tags`
- `library libraries`
- `library feeds`
- `library feed-items <library-id>`
- `library semantic-search <query>`
- `library semantic-index`
- `library semantic-status`
- `library duplicates`
- `library duplicates-merge`
- `library saved-search list`
- `library saved-search create`
- `library saved-search delete`

## search

`library search` 支持关键字搜索和结构化过滤组合。

常用示例：

```bash
zot --json library search "transformer attention" --limit 10
zot --json library search "reward hacking" --collection COLL001 --type preprint --limit 20
zot --json library search "attention" --tag attention --creator Vaswani --year 2017
zot --json library search "alignment" --sort date-added --direction desc
```

可用参数：

- `--collection <key>`
- `--type <item-type>`
- `--tag <tag>`
- `--creator <name>`
- `--year <yyyy 或前缀>`
- `--sort <date-added|date-modified|title|creator>`
- `--direction <asc|desc>`
- `--limit`
- `--offset`

## citation key、tags、libraries、feeds

```bash
zot --json library citekey Smith2024
zot --json library tags
zot --json library libraries
zot --json library feeds
zot --json library feed-items 3 --limit 20
```

说明：

- `citekey` 先走本地 Extra fallback；Better BibTeX 可用时会自动补强
- `library libraries` 可同时列出 user、group、feed library 概况
- feed 不通过 `--library` 切换，而是显式用 `library feeds` / `feed-items`

## semantic index / search / status

```bash
zot --json library semantic-status
zot --json library semantic-index --fulltext
zot --json library semantic-index --collection COLL001 --force-rebuild
zot --json library semantic-search "mechanistic interpretability" --mode hybrid --limit 10
```

支持模式：

- `bm25`
- `semantic`
- `hybrid`

说明：

- library-level semantic index 使用本地 sidecar 数据库
- 与 workspace 检索复用同一套索引实现，但不是同一个索引文件
- embedding 未配置时，不要假设 semantic / hybrid 一定可用
- `semantic-index` 默认走**替换式增量**：不加 `--force-rebuild` 时，只重建本次命中的条目，并清理库里已经删除的旧 key
- `--force-rebuild` 会在写入前清空整个索引文件，仅在需要彻底重建（例如换了 embedding 模型）时使用

## duplicates 与 merge

```bash
zot --json library duplicates --method both --limit 50
zot --json library duplicates --method title
zot --json library duplicates --method doi

zot --json library duplicates-merge --keeper KEEP001 --duplicate DUPE001 --duplicate DUPE002
zot --json library duplicates-merge --keeper KEEP001 --duplicate DUPE001 --duplicate DUPE002 --confirm
```

`duplicates-merge` 默认是 dry-run。只有加 `--confirm` 才会真正：

- 合并 tags
- 保留 / 补齐 collections
- re-parent child items
- 尝试跳过重复 attachment
- 把 duplicate 送入 Trash

## saved search

```bash
zot --json library saved-search list
zot --json library saved-search create --name "Recent RL" --conditions conditions.json
zot --json library saved-search delete SRCH0001
```

说明：

- `saved-search list` 返回的是保存查询的元数据和条件
- `saved-search create` 的 `--conditions` 可以是 JSON 字符串，也可以是 JSON 文件路径
- `saved-search delete` 删除的是保存查询本身，不会删除条目
- Zotero Web API 当前不直接返回 saved search 的结果集

## 推荐配合方式

典型顺序：

1. `library search` 或 `library citekey`
2. `item get`
3. `item cite` / `item export` / `item pdf` / `item children`

如果你不是在处理单篇，而是在围绕一批论文建立长期检索集合，转到 [workspace](/cli/workspace)。
