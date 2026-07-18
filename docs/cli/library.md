# library 命令

`library` 是默认的本地只读入口，负责“先查、再定位、再转到 item/workspace”这一层工作。

## 子命令

- `library search <query>`
- `library list`
- `library recent`
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
- `library dedupe`
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

## recent

`library recent` 现在有两种模式：

```bash
zot --json library recent --count 10
zot --json library recent 2026-04-01 --limit 20
```

说明：

- `--count <n>` 表示最近 N 条刚入库的条目，按 `dateAdded desc` 返回
- `<YYYY-MM-DD> --limit <n>` 表示取某个时间边界之后的条目
- 不带参数时，默认等价于 `library recent --count 10`

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

- 补 keeper 缺失的 metadata 字段
- 合并 tags
- 保留 / 补齐 collections
- re-parent child items
- 尝试跳过重复 attachment
- 给 keeper 写一条指向每个被并条目的 `dc:replaces` relation，Word / LibreOffice 里已插入的引文不会断链
- 把 duplicate 送入 Trash（可恢复，不做永久删除）

说明：

- 重复检测会跳过已在回收站里的条目
- 不同 item type 的条目可以合并；keeper 保持自身类型，只补对该类型合法的字段
- keeper 类型不支持的源字段会被跳过，并在 preview / applied 输出里以 `skipped_incompatible_fields`（字段名 + 来源 key）列出
- `dc:replaces` 的 URI 会在同一份输出里以 `relations_to_add` 给出
- preview/confirm 统一使用 Zotero Web API；确认前必须配置 `library_id` 与 `api_key`
- Web API 保持多请求、非事务写入语义

如果你不是从重复候选里合并，而是手里已经有两条明确的 key，改走 [item](/cli/item) 里的 `item merge`。要一次清理整库，用下面的 `library dedupe`。

## dedupe

`library dedupe` 是批量清理入口：一条命令完成检测重复组、每组自动选 keeper、输出整库或整 collection 的清理计划。

```bash
zot --json library dedupe
zot --json library dedupe --method doi --limit 100
zot --json library dedupe --collection COLL001
zot --json library dedupe --collection COLL001 --confirm
```

可用参数：

- `--method <both|doi|title>`（默认 `both`）
- `--collection <key>`
- `--limit <n>`（默认 50）
- `--confirm`
- `--include-low-confidence`

不带 `--confirm` 时是纯本地 dry-run：不触网，也不需要写凭据。计划 JSON 包含 `groups[]`、`total_groups`、`confirm_required`，每组给出：

- `match_type`：`doi`、`title`，或组合值如 `doi+title`——共享条目的检测组会先合并成一个连通分量，每个条目在计划中最多出现一次
- `confidence`：`normal` 或 `low`；`low` 组会附 `confidence_note`（年份差 > 1，或组内 DOI 互异），确认前值得人工看一眼
- `keeper`：保留下来的条目（`key`、`item_type`、`title`）
- `reason`：keeper 胜出的依据——先按类型优先级（journalArticle = conferencePaper > book / bookSection > thesis > report > preprint > document > 其他），再依次以非空元数据字段数、本地附件数、更早的 `dateAdded`、key 顺序做 tie-break
- `absorb`：被并入 keeper 并送入 Trash 的条目

`--confirm` 会按计划逐组执行，走与 `duplicates-merge` 相同的 selected backend，包括 `dc:replaces` 与跨类型字段安全。单组失败不会中断其余组。默认只执行 normal-confidence 组；low-confidence 进入 `skipped_low_confidence`，writer 不会收到这些组。结果包含 `applied`、`failed`、`skipped_low_confidence`、`total_groups`、`eligible_groups`、`applied_groups`、`failed_groups`、`skipped_low_confidence_groups`。

说明：

- 组内可以混合 item type（例如 preprint + conferencePaper）；keeper 保持自身类型
- 整库 `--confirm` 之前，先复查 `confidence: "low"` 的组，或先用单个 `--collection` 小范围试
- 普通 confirm 不授权 low-confidence。只有单独展示这些组并取得明确风险授权后，才使用 `--include-low-confidence`
- desktop 需要 Zotero 正在运行、bridge 已安装配对且目标库可写；desktop 失败不检查 Web credentials，也不 fallback
- 要对单个组手工指定 keeper，继续用 `library duplicates-merge`

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
