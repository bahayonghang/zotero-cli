# 本地单 key 删除分类

## 边界

改这些层：

- `zot-desktop`：在 `LocalApiClient` 上增加单对象 `delete_collection`。不改 probe、授权、SQLite 或 connector。
- `zot-cli`：`collection delete` 在 `--write-backend local` 时走新命令模块。默认仍走 `ctx.remote().delete_collection`。
- `skills/zot` 是 skill 源。镜像只由 `just skills-sync` 生成。
- 用户文档和 `.trellis/spec/zot-cli/backend/local-api.md`、`database-guidelines.md` 里把删除说成 Web-only 的句子要改成新契约。重命名和重挂父级保持 Web-only。

不改 `src/zot-remote` 的删除实现。不改 taxonomy 任务目录。

## 命令

```text
zot collection delete KEY
zot collection delete KEY --write-backend local
zot collection delete KEY --write-backend local --confirm \
  --expected-server-id ID \
  --expected-version N \
  --expected-descendants-sha256 HASH
```

`CollectionCommand::Delete` 从 `CollectionKeyArgs` 改为新的 `CollectionDeleteArgs`：位置参数仍是 `key`，再 flatten 一组只属于删除的本地参数。不要复用 `LocalCreateArgs`，那组参数绑的是库版本。

本地确认参数：

- `write_backend` 默认 `web`。
- `confirm` 要求三个 expected 参数同时存在。
- `expected_server_id`、`expected_version`、`expected_descendants_sha256` 都要求 `confirm`。
- `expected_version` 是 `Option<u64>`。`Some(0)` 必须保留，不能把缺省和 0 当成一回事。

Web 分支先拒绝任何 confirm / expected 参数，错误码 `local-write-options`，再调用现有远端删除。本地分支用 `Box::pin`，和 `collection create` 一样，避免 Windows 主线程栈过大。

key 必须是 8 位大写字母或数字，错误码 `local-api-key`。校验失败时不发请求。

## 预览

本地规划只使用 `ctx.local_api()`：

1. `probe`，取可写 Server ID。确认时若 `--expected-server-id` 不符，返回 `local-api-conflict`。
2. `collection_snapshot`。不调用 `ctx.local_library()`，不调用 `ctx.remote()`。
3. 目标 key 不在快照中：`local-write-target`。已标记 `deleted` 的目标仍可预览，报告 `already_trashed: true`，确认后仍是永久擦除。
4. 从 `parent` 建子索引，自目标做 BFS。子孙不含目标自己。快照里重复 key，或子树回到已访问 key：`local-api-protocol`。无关分支上的环不必展开。
5. 子孙按 key 字节升序。哈希输入是 UTF-8 key 用 `\n` 连接，没有末尾换行。空列表哈希空字节串，即 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。预览输出小写十六进制。比较时 trim 后必须是 64 位十六进制，再按 ASCII 忽略大小写比较。格式不对是 `local-write-options`，在授权前返回。
6. 子分类改名不改变哈希。新增、消失或改父级会改变哈希。

报告字段至少包括：`backend`=`local`，`operation`=`delete-collection`，`state`=`preview`，`server_id`，`library`，`library_version`（只展示，不作为确认条件），`collection_key`，`name`，`parent_key`，`version`，`already_trashed`，`descendants`（key、name、parent、version、deleted，按 key 排序），`descendant_count`，`descendants_sha256`，`effect`，`write_accepted`=`false`，`verified`=`false`。

`effect` 固定为英文，供测试断言：

`Permanently erases this collection and every descendant collection. Items stay in the library and lose only these memberships.`

预览不计算文献数量，因为规划不扫描条目。

## 确认与删除

确认复用同一次规划结果，在授权前检查 Server ID、`expected_version == report.version`、子孙哈希。不符分别使用：

- Server ID 或对象版本：`local-api-conflict`
- 子孙哈希：`local-delete-descendants`

两条都不读取授权、不调用 DELETE。

通过后沿用 `local_create` 的凭据加载和一次性 401 重授权。不要在日志或报告里写 key。

`delete_collection(server, scope, key, version, authorization)`：

- `DELETE {prefix}/collections/{key}`。用户库路径形如 `/api/users/0/collections/COLL0001`，群组走现有 `prefix`。
- 头：`Zotero-API-Key`、`Zotero-Server-ID`、`If-Unmodified-Since-Version`（对象版本的十进制，0 就是 `0`）。
- 无 body，无 `Zotero-Write-Token`，无查询串。

收据：

| 响应 | 收据 |
| --- | --- |
| 传输失败、5xx、HTTP 200、204 但 Server ID 缺失或不符 | `Unknown` |
| 204 且 Server ID 一致 | `Accepted` |
| 404 | `Absent` |
| 401、403、412、428、其他 4xx | `Err`，沿用 `check_status` |

不要把 204 映射成 Unknown。`create_collection` 把 204 当 Unknown 是因为创建成功是带 body 的 200；删除相反。

命令状态：

| 收据与回读 | state | 退出码 |
| --- | --- | --- |
| 未 confirm | `preview` | 0 |
| Accepted，随后 GET 是 `local-api-not-found` | `applied`，`verified=true`，`observed_expected_state=true`，`write_accepted=true` | 0 |
| Accepted，GET 200 或回读出现其他错误 | `verification-failed`，`verified=false` | 1 |
| Unknown | `unknown` | 1 |
| Absent | `absent` | 1 |

`unknown` 和 `absent` 不做验证性 GET。`unknown` 的 error code 用 `local-api-unknown`。`absent` 用 `local-delete-absent`，message 说明 DELETE 时对象已不存在，这次调用没有删除它。非成功状态的 envelope 仍是 `ok: true` 加非零退出码，与 `library organize` 一样，让 stdout 保留报告；skill 必须写明只看 `state` 与 `verified`。冲突和参数错误继续走 `ok: false`。

412 留在错误路径，不进入 `unknown`。

## 测试

桌面测试放在 `local_api_tests.rs`，沿用现有 mock：

- DELETE 路径、三个头，且没有 write token。
- 204 加一致 Server ID 为 Accepted。
- 500、HTTP 200、204 加错误 Server ID 为 Unknown。
- 404 为 Absent。
- 412 是 `local-api-conflict`，不是 Unknown。
- 版本 0 会发送 `If-Unmodified-Since-Version: 0`。

命令测试放在新的 `local_delete_tests.rs`，沿用 `local_create_tests.rs` 的 mock 方式：

- clap 能解析本地确认参数；`cli.rs::parses_new_library_and_item_command_surfaces` 覆盖这条 argv。
- 预览不出现 DELETE，也不要求授权。
- 哈希、版本、Server ID 不符时 mock 收不到 DELETE。
- 空子孙哈希是上面的常量。含已删除子孙；子孙 key 排序稳定。
- 204 后 GET 404 为 applied。204 后 GET 200 为 verification-failed。Unknown 不会因为后续 404 变成 applied。版本 0 可确认。

不运行真实 `zot collection delete`。

## Skill 与文档

`skills/zot/references/writes.md`：

- Local API 能力表和命令路由加入 `collection delete`。
- 7c 只保留“重命名和重挂父级仍只走 Web”。
- 新增 7e：本地删除的预览、三项确认、子孙永久删除、文献保留、成功状态。
- 安全门层 B 补一句：本地删除还会永久删除全部子分类。

`SKILL.md` 的 Local API 能力句加上单 key 分类删除。安全节写明确认三元组，以及不要替用户点授权框。

`evals.json` 与 `test-prompts.json` 用下一个空号 48。场景是没有 Web key，删除一把已知分类 key，先预览子孙再确认；有子分类时必须说出会永久删除子分类且文献仍在。

然后只跑 `just skills-sync`。不要手改 `.agents/skills` 或 `.claude/skills`。

同步改中英 CLI 文档：

- `docs/cli/collection.md`、`docs/en/cli/collection.md`：本地删除例子和级联说明。破坏性提醒保留。
- `docs/cli/local-write.md`、`docs/en/cli/local-write.md`：删掉“删除没有本地写路径”。重命名和重挂父级仍没有。
- `docs/skills/safety.md`、`docs/en/skills/safety.md`：Local API 写范围加入单 key 删除，层 B 保留 `collection delete`。
- `AGENTS.md` 的 mutation 段补一句：分类删除可以显式走本地预览/确认；重命名和重挂父级仍走 Web。不要自动换传输。

Spec 在实现符合契约后更新，避免检查时仍写着 Web-only：

- `local-api.md` 签名和“Collection rename/delete remain Web-only”。
- `database-guidelines.md` 的 `collection rename/delete` 只走 `ctx.remote()`。

## 兼容与回滚

旧的无 flag Web 删除不变。新 flag 只在显式 local 时生效。回滚是去掉本地分支和客户端方法，文档改回 Web-only。没有数据迁移。
