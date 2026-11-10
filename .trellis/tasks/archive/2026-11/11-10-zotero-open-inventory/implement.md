# 实现顺序

用户复审并明确批准本方案之前，不要运行 `task.py start`，不要改产品代码。

## 清单

1. 读 `.trellis/spec/zot-cli/backend/index.md` 的预检，以及 `desktop-read.md`、`local-api.md`、`database-guidelines.md`。
2. 为十条读命令加上 `--running-zotero`。`collection list` 和 `library tags` 今天没有参数结构，需要新的参数类型。写入命令不增加这个读开关。
3. 在 `zot-desktop` 增加完整读取分页：通过 `read_exchange` 发送 `limit=100` 和 `start`，Server ID 使用 `Option<&str>`。最多 100 页。普通 `DesktopScanLimits.max_pages` 仍是 20。不要改 `collection_snapshot` 的必填 Server ID 和 `local-api-conflict`。
4. 版本或 `Total-Results` 变化时整批重试，最多 3 次，然后 `desktop-read-inconsistent`。
5. 分类成员只请求 `/collections/{key}/items/top?includeTrashed=1&limit=100`。用这一页集合生成 `collection items`、`item-count` 和 `collection tags`。
6. `library list` 取完过滤结果后按 key 降序切页。分类参数先按 key、唯一名称和重名处理。
7. `library stats` 按 `get_stats_with_trashed` 的字段定义汇总。`library tags` 按 `get_tags()` 汇总，不复用书目页。
8. 快照分支使用 `Route::Snapshot` 中的库。测试重置 `snapshot_open_attempts`，桌面成功时期望增量为 0，回退时期望增量为 1。
9. 用 10.0.8 形状的模拟响应覆盖 AC3、AC4、AC5、AC8、AC9。用 `rich_fixture_library` 和扩展夹具覆盖 AC6、AC7 的两端逐值对照。
10. 修改 `zotero_db_busy_error` 的提示。快照错误码仍是 `zotero-db-busy`。
11. 更新 `skills/zot`、`desktop-read.md` 和 `docs/agents/limits.md`。中英文 CLI 文档只改这些命令真正变了的句子。
12. 跑下面的验证命令。不对本机库执行删除、移动或创建。

## 验证

```text
cargo test -p zot-desktop --lib local_read
cargo test -p zot-local --lib enumerates_tags_libraries_feeds_and_feed_items
cargo test -p zot-cli --bin zot collection
cargo test -p zot-cli --bin zot library
just ci
```

`just ci` 是合并前的总闸。模拟服务器只证明客户端发出的路径、`limit` 和 Server ID。期望计数来自同一夹具上的现有 SQLite 函数，或来自上面记录的 10.0.8 响应形状。

手动记录可以在实现合并前另做：本机打开 Zotero，确认 `collection list` 和 `library list --limit 2500` 的 `read_source` 为桌面，且 `snapshot_open_attempts` 不增加。该记录不进入 `just ci`。未做时标成待验。

## 风险文件

- `src/zot-cli/src/commands/collection.rs`：读分支今天全部依赖快照。
- `src/zot-cli/src/commands/library.rs`：只改 list、stats、tags。`search` 的 `snapshot_first` 不要动。
- `src/zot-cli/src/cli/args.rs`：十条读命令需要 `--running-zotero`。
- `src/zot-desktop/src/local_read.rs`：不要把默认 `max_pages` 从 20 改掉。
- `src/zot-desktop/src/local_api.rs`：读分页可以邻近 `read_exchange`。`collection_snapshot` 保持写入预览行为。
- `src/zot-local/src/db.rs`：不改 `get_tags`、统计 SQL 和忙等时长。只允许改忙错误的提示文字。
- `skills/zot/SKILL.md`：权威 skill。`.agents/skills` 和 `.claude/skills` 是生成镜像。

## 回滚点

第 5 步到第 8 步是行为开关。对照测试失败时先恢复路由，再保留尚未接入的分页器。规格和 skill 不能单独超前于路由。
