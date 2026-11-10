# 实现顺序

1. 读 `prd.md`、`design.md`、`research/erase-semantics.md`，以及 jsonl 指向的 spec。不要读或修改 `.trellis/tasks/11-09-zotero-collection-taxonomy`。
2. 在 `src/zot-desktop/src/local_api.rs` 增加 `DeleteReceipt` 和 `delete_collection`。204 是 Accepted，不要复制 `create_collection` 的 204→Unknown。补 `local_api_tests.rs`。
3. 在 `args.rs` 增加 `CollectionDeleteArgs`。`collection.rs` 按 backend 分支。Web 仍调用现有 `delete_collection`。
4. 新增 `commands/local_delete.rs` 与 `local_delete_tests.rs`，并在 `commands/mod.rs` 注册。规划与确认分开。哈希助手写在本模块，不要导出 `organize::sha256_hex`。
5. 把本地删除 argv 加进 `cli.rs::parses_new_library_and_item_command_surfaces`。
6. 更新 `skills/zot/SKILL.md`、`references/writes.md`、`evals/evals.json`、`test-prompts.json`。运行 `just skills-sync`。
7. 更新 design.md 点名的中英文档、`AGENTS.md`，以及两份 spec 里已经失真的删除句子。不要顺手改重命名或重挂父级的行为。
8. 先跑：
   - `cargo test -p zot-desktop --lib local_api_tests --locked`
   - `cargo test -p zot-cli --lib local_delete --locked`
   - `cargo test -p zot-cli --lib parses_new_library_and_item_command_surfaces --locked`
9. 上述通过后再跑 `just ci`。不要用会改文件的 recipe 代替它。
10. 不对本机 Zotero 执行删除。不 commit，不 push，不创建 GitHub issue。

## 审查

- 预览路径没有授权调用，也没有 SQLite。
- 确认三条件都在 DELETE 之前。
- `unknown` 分支没有 GET。
- Web 默认行为没变。
- skill 镜像与 `skills/zot` 一致。
- 工作区 lint：无 `unsafe`、`dbg!`、`todo!`、`unwrap()`。

## 回滚点

桌面客户端方法、CLI 分支、文档/skill 可以按层撤回。没有库内数据步骤。
