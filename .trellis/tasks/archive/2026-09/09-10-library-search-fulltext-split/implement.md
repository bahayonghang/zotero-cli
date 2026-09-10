# Implement: library search 兼容全文拆分

## Ordered Checklist

1. [x] `zot-core`：`EnvelopeMeta.fulltext_index`、`SearchResult.fulltext_index`；envelope 单测断言缺省省略、赋值 round-trip、`api_version=1`。
2. [x] `zot-cli`：`EnvelopeMetaSeed` / `CommandOutput::new` / `format.rs` 构造处接上该字段；仅 `library search` 在非空 query 时填入。
3. [x] `LocalLibrary::search`：`table_exists` 决定是否拼词表 LIKE；缺表时 metadata OR 必须可 prepare。无 sidecar 夹具先红后绿。
4. [x] 惰性 sidecar：`TempDir/fulltext.sqlite` Backup + `ATTACH AS ftindex`；busy/缺失/integrity → 跳过；OnceCell 记住结果。测试禁止 ATTACH live 路径。
5. [x] MATCH 构造器 + FTS EXISTS（`fulltextContent` / `fulltextContentCJK`，`rowid = itemAttachments.itemID`，`parentItemID = i.itemID`）。迷你 sidecar 夹具覆盖附件词命中父条目。
6. [x] doctor：`local_sqlite_read.fulltext.{legacy_tables,sidecar_present}`；`available` 不因缺词表/缺 sidecar 变 false；human 打印一行。
7. [x] 文档与 spec：`database-guidelines.md`、`error-handling.md`、`docs/agents/limits.md`、`docs/cli/library.md`、`skills/zot/SKILL.md`，然后 `just skills-sync`。
8. [x] `cargo test -p zot-core`、`cargo test -p zot-local`、`cargo test -p zot-cli` 相关用例，最后 `just ci`。本机 live search 在 Zotero 占用库时返回既有 `zotero-db-busy`；夹具覆盖不再出现 `search-count` / `no such table: fulltextItemWords`。

## Focused Validation

```powershell
cargo test -p zot-core envelope
cargo test -p zot-local db::tests
cargo test -p zot-local --test search_regression
cargo test -p zot-cli doctor
cargo test -p zot-cli format
```

本机（实施后，不在规划阶段执行产品修复）：

```powershell
zot --json library search IEEE --limit 3
zot --json doctor
```

## Full Gate

```powershell
just ci
```

## Risk And Rollback Points

- COUNT 与 page 必须使用同一套 predicates / 同一全文后端，否则 `total` 与当前页不一致。
- MATCH 字符串必须绑定。用户输入里的 `"`、`*`、`AND` 不得进入原始 FTS 语法。
- sidecar Backup 失败不得改写主库 `zotero-db-busy` 语义。
- 禁止 ATTACH `data_dir/fulltext.sqlite`。测试用路径断言或注入假源路径覆盖。
- `EnvelopeMeta` 所有字面量构造点都要加字段，否则不能编译；错误 envelope 保持 `fulltext_index=None`。
- 不改 `just ci` 纯检查属性；skill 镜像只经 `skills-sync`。
- 回退：还原本任务提交。不要只删 FTS 而留下无探测的词表 JOIN。

## Follow-up before `task.py start`

- `prd.md` 无 Open Questions。
- `design.md` / `implement.md` 已齐。
- `implement.jsonl` / `check.jsonl` 已有真实 spec/research 行。
- 用户已批准本轮最终规划摘要。
