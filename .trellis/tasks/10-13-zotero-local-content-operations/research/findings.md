# Zotero 内容操作能力调研

检索和本机验证日期：2026-10-13（America/New_York）。仓库基线：`dev` / `6751e48`，开始时工作区干净。这里只读调查；没有执行 authorize、POST/PATCH/PUT/DELETE，没有修改真实条目。

## 1. 结论

推荐接入 **Zotero 10+ 官方 Local API**。本机已运行 10.0.6，满足版本条件。现有 CLI 的问题是尚未接入这一写路径，并非 Zotero 仍然不能接受本地写入。

此前仓库 2026-07-18 的 connector-replace-bridge 任务移除了自研插件，采用 Connector 新增导入 + Web API 修改；当时文档把 `/api/` 视为只读。官方发布记录显示 10.0 在 2026-08-17 发布并增加 Local API 写支持，10.0.5 又修复 client-supplied key 的写问题。[S2] 因此应基于新的上游能力更新设计，无需恢复旧插件。

本轮没有证明真实写入成功。已确认的是当前官方契约、对应版本源码及本机只读 API 能力，授权和写入留给隔离测试库验收。

## 2. 现场证据

| 检查 | 结果 | 解释 |
| --- | --- | --- |
| `zot --version` | `zot 1.0.1` | 本轮 CLI 固定使用已安装 `zot` |
| `zot --json doctor` | Zotero `10.0.6`；Local HTTP/Connector available | doctor 仍只暴露 local read 和 connector import 能力 |
| Web credentials | `configured=false` | 当前已有 Web 写命令缺少凭据；没有验证远端权限 |
| Local SQLite | `zotero-db-busy`，consistent snapshot 超时 | 不等同于整个 Zotero 不可用；本轮不修复 |
| `GET http://127.0.0.1:23119/api/` | HTTP 200；API 3；schema 44；Server ID 存在 | curl 只读验证；没有取得任何 API key |
| `GET /api/itemTypeFields?itemType=conferencePaper` | HTTP 200，含 `citationKey`、`proceedingsTitle`、`conferenceName` 等 | 当前字段表必须以当前 schema 为准 |

PowerShell `Invoke-WebRequest` 读取根端点曾报 `ResponseEnded`；改用 `curl.exe --noproxy '*' --http1.1` 后成功。未确定该客户端差异的根因，不据此判断服务不支持写。doctor 还提示旧 bridge 配置残留，本轮没有读取或修改其中的秘密。

## 3. 当前代码能力与缺口

| 操作 | 当前实现证据 | 缺口 |
| --- | --- | --- |
| 分类新增/改名/删除/添加和移除成员 | `src/zot-cli/src/commands/collection.rs:9`；`src/zot-remote/src/zotero.rs:298` | 全部 Web；没有专门 move 操作 |
| 修改字段 | `src/zot-cli/src/commands/item/write.rs:124`；`src/zot-remote/src/zotero.rs:161` | 字符串 map，GET 后 PUT；无 local preview、fill-empty、结构化作者输入 |
| 字段参数 | `src/zot-cli/src/cli/args.rs:511` | 只有 title/date/重复 field；缺 `=` 的 field 当前被跳过 |
| 分类成员 | `src/zot-remote/src/zotero.rs:355`、`:385` | 各自 GET 后 PATCH；连跑 add/remove 不提供一次移动的单对象一致性 |
| notes/tags | `src/zot-cli/src/commands/item/note.rs:11`、`tag.rs:28` | 已有 Web 能力，不需把它们说成从零实现 |
| 附件/批注/回收站 | `src/zot-cli/src/commands/item/write.rs:145`、`:162`；`annotation.rs:122` | Web 写；部分准备阶段还依赖 SQLite/PDF |
| 发表信息更新 | `src/zot-cli/src/commands/sync.rs:13`、`:62` | 针对 preprint 状态，不是通用元数据补全器 |
| DOI/arXiv 元数据 | `src/zot-cli/src/commands/item/write.rs:178`、`:351`；`src/zot-remote/src/oa.rs` | 可复用检索/映射，但现有流程主要创建新条目 |
| 写入口 | `src/zot-cli/src/context.rs:70` | 强制 Web library id/key；本地 user 库不应经过此门 |
| Desktop crate | `src/zot-desktop/src/lib.rs:1`；`connector.rs:155` | 只有 Connector 和 Local API 探测，没有本地 mutation client |
| doctor | `src/zot-cli/src/commands/doctor.rs:52`、`:231` | 未区分本地写支持与授权；Web configured 也不等于 verified |

现有 `ZOT_ZOTERO_API_BASE` 实现细节不能拿来把 Web client 偷换成本地 client：本地还需要独立授权、实例标识和本地版本；密钥与数据源混用有实际语义错误。

## 4. 写路径比较

| 路径 | 能力及条件 | 适合程度 |
| --- | --- | --- |
| 官方 Local API | Zotero 10+，应用运行，启用本地通信，用户在 Zotero 授权；本地修改由应用处理 | **首选**：已有条目操作、离线运行和桌面反馈 [S1] |
| 官方 Web API | 网络、Web key、对应库权限；云端改动经同步到桌面 | 保留兼容，适合远程/无桌面的流程；不满足严格离线场景 [S3,S7] |
| 内置 Connector | 本项目只使用 BibTeX/RIS import 和当前目标探测 | 保持新增导入用途，不能作为现有 item 更新后端；见现有 `connector.md` |
| JavaScript API / 插件 | Zotero 应用内通过对象 API 和保存事务修改；需应用内执行或维护插件 | 旧版本/特殊能力备选；常规首版操作无需重建插件桥 [S5,S8] |
| UI 自动化 | 模拟当前 UI 操作 | 少量人工辅助；焦点/版本/语言变化使其不适合作为 CLI 主后端（设计判断） |
| 直接 SQLite UPDATE | 绕过 Zotero 应用对象、通知和同步生命周期 | 不采用；遵守仓库只读边界 |

Local API 的官方差异：[S1]

- 本地 key 由 `/api/local/authorize` 的用户对话框授予，区别于 Web key；支持一次或记住授权，key 不按 library 限权。
- 写请求必须绑定 `Zotero-Server-ID`；本地对象版本与 Web/其他实例的版本无可比性。
- items、collections、searches 的写方法及附件等能力已有支持；桌面可见的本地修改随后按正常同步流程上传。

对应 10.0.6 源码进一步支持以下判断：[S4]

- item/collection/search 单对象路由注册了 PUT/PATCH/DELETE；保存经过 Zotero 对象 API。
- 单次 key 在认证时消耗，后续校验失败也不能假设 key 可重用。
- `writeMultipleObjects` 按对象保存并分别记录结果，批量写不应宣称全有或全无。
- 字段转换使用非 strict 模式，CLI 校验和写后回读有价值；不能把任意字段的 2xx 当成已保存。

## 5. 操作矩阵

| 场景 | 正确语义 / 实现方向 | 首版 |
| --- | --- | --- |
| 添加分类 | 读取当前 collections，加入目标，其他归属保留 | 是 |
| 移除分类 | 只移除指定成员关系；不 trash、不删库中条目 | 是 |
| A→B 移动 | `C' = (C − {A}) ∪ {B}`；一次 PATCH 同一 item | 是 |
| 分类结构管理 | collection name/parentCollection 的独立修改，不等于移动 item | 后续 |
| 补充普通字段 | 按 itemType 校验，只提交目标字段；填空与覆盖分开 | 是 |
| 作者 | creators 是有顺序的对象数组；作者/编辑/机构姓名不能用字符串替代 | 是 |
| 标签 | 区分加/删和整体替换，保留自动/手动标签类型 | 后续 |
| 笔记 | note 是 item；child note 用 parentItem，内容与格式需专门处理 | 后续 |
| 关联条目 | relations 需保持合法关系及同库/跨库含义 | 后续 |
| 附件 | 区分元数据、文件上传、父子关系；不能移动 storage 文件冒充 API 操作 | 后续 |
| 批注 | attachment 下的 annotation item，包含页码、坐标等验证 | 后续 |
| 保存搜索 | 修改条件而非向搜索“移动 item” | 后续 |
| 回收站 | deleted 状态与永久 DELETE 分开 | 后续 |
| merge/dedupe | 多对象关系和引用身份保护；有 CRUD 不代表有原生原子 merge 端点 | 后续 |
| 跨库搬迁 | 复制/新身份/children/附件/权限/来源清理的组合，不是改 collections | 后续 |
| 联网补全 | 现有 DOI/arXiv/S2 作候选证据来源，先字段 diff 再写；不从标题猜 DOI | 后续 |

分类是多重成员关系，同一 item 可属于多个 collection，并不会因此复制条目；My Library 仍包含该条目。[S6] 因此“移到其他分类”必须指明源与目标，不能把全部原分类清空。

PATCH 保留未提交属性，但数组提交的是完整列表；PUT 漏掉字段可能移除数据；更新使用对象版本前置条件。[S3] 首版应生成最小 PATCH，不能将 CLI 的精简读模型重新序列化为整条 PUT。

## 6. 对截图 Info 面板的映射

截图所选的是 conferencePaper。截图只用于理解工作流，不用于确定实际 item key 或自动补写值。

| UI 信息 | API 数据 | 处理 |
| --- | --- | --- |
| Title / Date / DOI / URL | title / date / DOI / url | 字符串；字段大小写须准确 |
| Proceedings Title / Conference Name | proceedingsTitle / conferenceName | 不把期刊的 publicationTitle 固定套在会议类型上 |
| Publisher / Pages / Language / Short Title | publisher / pages / language / shortTitle | 当前类型允许时补写 |
| Author | creators | 数组；个人姓名和机构姓名两种结构 |
| Citation Key | 本机 schema 已返回 citationKey | 不再一概断言是 BBT 私有字段；与 item key 区分，BBT 自动生成/固定行为另行验证 |
| Item Type | itemType | 转换可能涉及字段/作者角色损失，首版排除 |
| Extra | extra | 保留已有内容；首版不提供隐式按行合并 |

字段和作者类型应从目标 API 的 schema/template 获取；官方提供 itemTypes、items/new、itemTypeFields、itemTypeCreatorTypes 等读取面。[S9] 本机 schema 探测比旧截图说明或旧 skill 中的固定清单更可靠。BBT 的生成/固定键规则属于另一层集成，不在首版承诺中。[S10]

## 7. 分期与验证边界

用户选择：第一期本地分类 + 手动字段/作者；第二期再考虑标签/笔记；第三期评估联网自动补全、批量操作和附件等。后两期只是调研建议，没有创建子任务或承诺实现。

第一期共享一个本地授权/读写/冲突闭环，保留一个任务，按 implement.md 顺序完成，避免为调研立即拆出空壳子任务。

本轮尚未测试本地授权对话框、真实修改、重启后的授权复用、只读 group、同步冲突或 UI Undo。SQLite snapshot busy 不阻塞 HTTP 方案，但需在真实验收中证明两者解耦。

## Sources

全部于 2026-10-13 检索，仅将官方文档/上游代码用作技术事实来源。

- S1：[Zotero Local API](https://www.zotero.org/support/dev/web_api/v3/local_api)，页面更新 2026-07-29；授权、实例/版本与本地写差异。
- S2：[Zotero Version History](https://www.zotero.org/support/changelog)，10.0/10.0.5/10.0.6 发布记录。
- S3：[Web API Write Requests](https://www.zotero.org/support/dev/web_api/v3/write_requests)，PATCH/PUT、数组、前置条件和对象写方法。
- S4：[Zotero 10.0.6 server_localAPI.js](https://github.com/zotero/zotero/blob/10.0.6/chrome/content/zotero/xpcom/server/server_localAPI.js)，重点 consumeLocalAPIKey、LocalAPI.Item、applyJSONToObject、writeMultipleObjects、writeSingleObject；固定 tag，不把 master 当本机版本。
- S5：[Zotero JavaScript API](https://www.zotero.org/support/dev/client_coding/javascript_api)，对象字段/作者及事务式保存。
- S6：[Collections and Tags](https://www.zotero.org/support/collections_and_tags)，多分类成员语义。
- S7：[Syncing](https://www.zotero.org/support/sync)，本地与在线库同步。
- S8：[Plugin Development](https://www.zotero.org/support/dev/client_coding/plugin_development)，插件替代路线。
- S9：[Item Type/Field Requests](https://www.zotero.org/support/dev/web_api/v3/types_and_fields)，schema/template/creatorTypes。
- S10：[Better BibTeX Citation Keys](https://retorque.re/zotero-better-bibtex/citing/)，键生成规则；不据此覆盖本机原生 schema 事实。
