# Design — 官方 Local API 内容操作

状态：用户于 2026-10-13 明确批准实施，代码和自动化验证已完成，真实授权写入验收待完成。依据见 [research/findings.md](research/findings.md)。

## 1. 路线与兼容策略

- 增加 zot-desktop 的 Local API client，将 Connector 和 Local API 作为独立能力。SQLite 只读边界不变。
- 在首版覆盖的 item update、collection add-item/remove-item/move-item 上显式选择 `--write-backend local|web`。既有命令省略参数仍按 Web 处理；新 move-item 要求显式 local，首版不实现 Web move。
- 选择 local 后，预览、schema 校验、版本读取、写和回读全部走同一 Local API。不因本地快照 busy 而先失败，不自动换后端。
- 普通 user 库使用 `/api/users/0`；group 使用显式公共 group id，不传 SQLite 内部 libraryID。不跨 library 移动。
- 协议条件为 Zotero 10+、API v3、有效 Server ID；首个真实兼容性基线为本机 10.0.6。旧版只读/Connector/Web 路径保持可用。

## 2. 最小职责划分

| 位置 | 职责 |
| --- | --- |
| `src/zot-desktop/src/local_api.rs`（新） | loopback HTTP、probe、authorize、目标 JSON/schema 读取、单对象 PATCH、回读、错误映射 |
| `src/zot-desktop/src/lib.rs` | 导出 client 和必要 DTO |
| `src/zot-core/src/config.rs` / `error.rs` | 必要共享秘密/错误契约，沿用 SecretString 和 envelope；不混入 Web credentials |
| `src/zot-cli/src/context.rs` | 构造 local client；编排独立本地授权状态的读取/保存 |
| CLI args、item/write、collection | 参数解析、填空/覆盖与集合运算、预览/确认；每次操作只改一个 item |
| `commands/doctor.rs` | 增量暴露 local write 能力，不授权、不试写 |
| skills/docs/spec | 自然语言路由、准确边界、版本条件与示例 |

不建立覆盖全部操作的通用 mutation 框架，不重构整个 ZoteroRemote；只复用必要纯逻辑和测试模式。

## 3. 授权与实例身份

1. 只读 GET /api/ 检查可达、版本、schema 和 Server ID；未知/缺失能力不视作可写。
2. 完成参数/目标校验和 diff。无变化则 unchanged，不请求授权。
3. 仅已确认 apply 才请求 `/api/local/authorize`，应用名为 `zot CLI`。操作确认和 Zotero 自身 Allow/Deny 分别处理。
4. 本地 key 只发给 loopback Local API。client 禁止非 loopback 跳转并绕过代理，不提供任意远端写地址。
5. 一次授权 key 仅保留进程内。用户在 Zotero 选择记住授权时，才保存到 AppConfig::state_dir() 下专用授权文件，按 CLI profile + Server ID 分区。沿用秘密封装，Debug/JSON/日志不泄露。不从 Zotero profile 私自读取授权文件。
6. 401 表示授权不可用；仅在已确认操作里允许一次重新授权。拒绝即停止；429 遵守 Retry-After，不循环刷对话框。版本冲突不触发再授权重试。

本地 key 无 library scope，CLI 仍需检查显式 library/target；网络隔离不能代替操作授权。

## 4. 预览与确认

本地操作不带 `--confirm` 时只返回计划，至少包含 backend、server_id、library、item_key、expected_version、operation、before/after、skipped_fields。人类文案展示标题与分类名称，agent 使用 key。

本地 apply 要求同一操作参数、`--confirm` 以及预览给出的 `--expected-server-id`、`--expected-version`。首版是单对象操作，不引入持久化队列或通用 plan token 系统。

- apply 重新读取目标并检查两个 expected 值，再提交条件 PATCH。
- 源/目标 collection 须同库、存在、非已删除，apply 再检查。名称仅显示，key 决定目标；UI 当前选中位置不影响写入。
- 其他编辑导致 conflict，提示重新预览，不自动合并或重试。
- 版本来自 Local API，不用 SQLite/Web 版本。payload 同时携带对象本地 version 与 If-Unmodified-Since-Version；前者在当前上游中作精确匹配。Server ID 贯穿读写。
- 首版不支持一次写多个 item；后续批量必须处理逐项结果。

## 5. 分类成员

当前分类列表 C，源 A，目标 B：

- add：保留 C 顺序，B 不存在时追加。
- remove：只移除 A；不在 A 中返回 unchanged。
- move：`C' = (C − {A}) ∪ {B}`，保持其他成员顺序；一次 PATCH 只提交 collections 与版本。
- A=B：unchanged；目标已含 B 时仅移除 A。
- 源不在 C、目标已存在：already-at-target / unchanged；源和目标均不在 C：source-membership-missing，不能把 move 偷换成 add。
- 只接受同库顶层普通条目；child note/attachment、跨库 key、已删除目标等写前拒绝。首版不改 parentItem、itemType 或 children。

只承诺单 item 一次提交，不承诺跨条目事务或 collection 元数据锁。目标分类在校验后被并发修改/删除时，须检查服务器结果与回读差异。

## 6. 字段与作者

- 延续 `--title`、`--date`、`--field NAME=VALUE`。local 分支对缺 `=`、未知字段、重复冲突赋值、空字段名报错，不静默跳过。
- 允许字段取自目标当前 schema/template；payload 使用稳定字段 ID，不使用 UI 本地化 label。
- 默认 fill-empty：字段不存在、空字符串或仅空白时填入；非空且不同的字段保留并列入 skipped。`--overwrite` 显式允许替换指定非空字段。等值输入不写。
- 空字符串不作为补充值；首版不增加通用清空动作。extra 视作整体字符串，不隐式删除/拼接已有行。
- `--creators-file <json>` 接受完整、有顺序的 creators 数组。个人 firstName/lastName 与机构 name 互斥；creatorType 须适用于 itemType。全空姓名、空数组、非数组、错误结构均拒绝。
- creators 仅在原数组为空时默认补入；已有数组须 --overwrite 才替换。完整展示新列表；不模糊去重、自动拼接或猜测姓名分割。
- itemType、key、version、library、parentItem、tags、collections、relations、deleted 等结构/身份字段不能通过 --field 修改；分类走专门动作。
- native citationKey 若 schema 支持，可显式编辑；它不是 item key。BBT 自动重算不由本功能承诺，回读不一致须报告；未请求时 CLI 不主动改键。

## 7. 结果、错误与恢复

- applied：响应成功且同实例回读匹配目标字段/成员。只比较本次字段，允许 Zotero 正常更新时间等。
- unchanged：无变化，未写。
- conflict：实例/版本/成员前提变化，未发送更新或由服务器拒绝。
- failed：明确拒绝/无权限/无效字段/目标不存在；稳定错误码，不暴露密钥。
- unknown：发送后断连，不能判断是否应用。先同实例回读；即便观察到期望值，也区分“观察到期望状态”和“确认此请求成功”，不重复发送。
- 响应成功但回读失败：报告 write accepted + verification failed，不简化为未写入。

不承诺 UI Undo。恢复需用户复核 before 值后，以当前版本作反向修改；不盲目回滚或实现通用撤销引擎。

## 8. 契约迁移

实施时有意修订 AGENTS、skills/zot/SKILL.md、connector spec 和有关文档中“Local HTTP 始终只读”的表述，增加经授权的 Zotero 10+ Local API。Connector import-only、SQLite read-only 继续保留。

doctor 增加 local_api_write：reachable、supported、zotero_version、api_version、schema_version、server_id、credential_present、verified=false/reason。检查不到权限不能声称 editable；探测或凭据存在不等于实际写成功。

不把 tags/notes/merge 偷偷迁往 local。既有 Web 默认语义保持；local 专用覆盖/预览/expected 参数与 Web 组合时明确拒绝，不忽略。

## 9. 延后事项

批量写、自动补全、tags/notes/attachments、分类结构管理、跨库迁移另行规划。UI Undo、BBT 重算与真实 group 权限需要测试证据；首版不承诺未验证行为。
