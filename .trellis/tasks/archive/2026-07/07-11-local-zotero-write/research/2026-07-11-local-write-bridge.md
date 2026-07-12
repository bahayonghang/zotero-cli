# Zotero 本机安全写入桥接调研（2026-07-11）

## 结论

当前 `zot` 要实现“无 Web API key、直接作用于正在运行的本机 Zotero”时，
可行且可自动化的稳健路径是：随仓库提供一个最小 Zotero 插件，在
`127.0.0.1:23119` 注册版本化、带本地 bearer token 的白名单 JSON 端点；
Rust CLI 通过独立 desktop bridge client 调用该端点，插件在 Zotero 客户端
事务内调用受支持的 JavaScript API。Local HTTP API 继续只读，SQLite 继续
只读。

一次性 Run JavaScript 脚本可以作为诊断或应急 fallback，但不能成为 CLI 的
默认执行后端：它需要人工复制运行、无法可靠返回结构化结果、难以实施版本
握手和并发校验，也不能满足 agent 端到端执行的目标。

## 当前环境与仓库证据

- 本机运行 `C:\Program Files\Zotero\zotero.exe`，版本 `9.0.6`。
- `http://localhost:23119/api/` 返回 Zotero `9.0.6`、Connector API `3`，证明
  本地服务可用。
- `src/zot-cli/src/context.rs:61` 的 `AppContext::remote()` 强制要求
  `library_id` 和 API key；`src/zot-cli/src/commands/item/merge.rs:34` 的
  `merge_item_set` 只接受 `ZoteroRemote`。
- `library dedupe` 的检测、keeper 评分和 confidence 计划已经是本地纯函数；
  `item merge` / `duplicates-merge` / `dedupe --confirm` 共享同一合并引擎。
- `skills/zot/SKILL.md` 是 canonical source。`just install` 的
  `_install-skills` 将 `skills/*` 复制到 `.agents/skills` 和 `.claude/skills`；
  当前 `.agents/skills/zot/SKILL.md` 只是安装镜像，不应作为独立编辑源。
- `skills/zot/evals/evals.json` 与 `test-prompts.json` 已有 26 条路由回归，但
  现有写入用例仍把“写能力”等同于 Web API 凭据，需要改成本机/远程后端分流。

## Zotero 官方能力（9.0.6 tag）

### Local HTTP API

官方文档与 `server_localAPI.js` 一致：本地 API 不认证、离线、仅接受 GET，
写请求当前不支持。它适合读，不适合承担本任务的 mutation transport。

来源：

- https://www.zotero.org/support/dev/web_api/v3/local_api
- https://github.com/zotero/zotero/blob/9.0.6/chrome/content/zotero/xpcom/server/server_localAPI.js

### 插件与自定义 loopback endpoint

Zotero 插件运行在桌面客户端内部，可访问 JavaScript API。Zotero 的内部
server 通过 `Zotero.Server.Endpoints[path] = class { ... }` 注册端点，端点可
限制 `supportedMethods`、`supportedDataTypes` 和 bookmarklet 访问，并在
shutdown 时删除注册。

Better BibTeX 的 debug bridge 证明该机制在真实插件中可用，也展示了
`Authorization: Bearer <token>` 的端点认证模式；但它执行任意 JavaScript，
本任务只能借鉴 transport 和认证，不能复用其任意代码执行能力。

来源：

- https://www.zotero.org/support/dev/client_coding/plugin_development
- https://github.com/zotero/zotero/blob/9.0.6/chrome/content/zotero/xpcom/server/server.js
- https://github.com/retorquere/zotero-better-bibtex/tree/master/test/fixtures/debug-bridge

### 原生 merge 语义

`mergeItems.mjs` 的 `mergeItems(master, others)`：

1. 以 `Zotero.DB.executeTransaction()` 包住整个操作；异常会回滚。
2. 只要求所有条目处于同一 library，不要求相同 item type。
3. 合并/迁移 PDF、网页附件、其他附件、notes、relations、collections、tags。
4. 写 `dc:replaces`，修复 notes 内 item key，并对重复附件做比现有 CLI 更完整
   的 hash、全文、annotation 和 link-mode 判断。
5. 将 loser 标为 deleted 并保存。
6. 不自动吸收 bibliographic metadata 字段；GUI 在调用 merge 后端前负责用户
   选择。CLI 本机后端必须显式制定“仅填 keeper 合法且为空字段”的计划。

因此，本机后端应复用原生 merge 的结构性语义，而不是在插件里复制附件、
note 和 relation 迁移算法。metadata fill 应在同一次事务中先应用到内存中的
keeper，再调用原生 merge。

来源：

- https://github.com/zotero/zotero/blob/9.0.6/chrome/content/zotero/mergeItems.mjs
- https://github.com/zotero/zotero/blob/9.0.6/chrome/content/zotero/xpcom/data/items.js

### SQLite

官方明确要求直接 SQLite 访问只读；写入会绕过校验和引用完整性并可能损坏库。

来源：https://www.zotero.org/support/dev/client_coding/direct_sqlite_database_access

## 候选方案比较

| 方案 | 自动化 | 安全边界 | 事务/引文 | 安装成本 | 结论 |
| --- | --- | --- | --- | --- | --- |
| Web API | 高 | API key + 云端 | 无跨请求事务，现有自研 merge | 配 key | 保留为显式 remote 后端 |
| Run JavaScript | 低 | 人工审查脚本 | 可用原生事务 | 无插件 | 仅 fallback |
| 任意 JS debug bridge | 高 | bearer token 但能力过宽 | 取决于脚本 | 装第三方插件 | 拒绝 |
| 最小白名单插件桥 | 高 | bearer token + 固定 DTO + POST-only | 原生事务/merge | 一次安装 | 推荐 |
| 直接 SQLite 写 | 高 | 绕过 Zotero | 破坏完整性风险 | 无 | 禁止 |

## 推荐架构

```text
natural-language request
  -> skills/zot/SKILL.md backend routing + safety gate
  -> zot-cli command / dry-run / confirmation
  -> zot-desktop Rust client (versioned DTO, auth, timeout, error mapping)
  -> POST http://127.0.0.1:23119/zot-bridge/v1/<operation>
  -> Zotero plugin (schema validation, library/key/version validation)
  -> Zotero JavaScript API transaction / native mergeItems
  -> structured result -> standard zot JSON envelope
```

建议新增 `src/zot-desktop` crate，而不是把桥接客户端塞进 `zot-cli` 或
`zot-remote`：认证、协议版本、loopback HTTP、重试和 fake-server 测试构成一个
真实复杂度边界；`zot-remote` 继续只表示云端服务，`zot-local` 继续只负责
SQLite/PDF/workspace。

插件建议放在 `plugins/zot-bridge/`，产出一个 XPI。Rust CLI 与插件共享文档化
的 protocol v1 JSON contract，但插件不接受脚本、字段级任意 mutation 或任意
endpoint 名称。

## 协议和安全约束

- 仅绑定 Zotero 已有的 loopback server；endpoint 仅支持 POST + JSON。
- 所有 mutation 请求要求 bearer token；token 不写日志/错误/envelope。
- `permitBookmarklet = false`，拒绝带 browser `Origin` 的写请求；Authorization
  不在 Zotero 默认 CORS allow-header 中，可进一步阻断网页跨域调用。
- 请求体有严格大小上限、schema、协议版本和 operation enum。
- 第一阶段只暴露 `status`、`merge-preview`、`merge-apply` 等白名单操作；不提供
  eval/execute/script endpoint。
- preview 返回条目版本/指纹和短期 plan token；apply 重算并比对，漂移则拒绝。
- merge apply 通过原生 `mergeItems()` 保证组内事务；批量仍由 CLI 逐组调用并
  汇总部分失败。
- 重试时根据 loser deleted 状态和 keeper `dc:replaces` 判断已应用，避免响应
  丢失导致重复写。
- 插件启动时注册端点，shutdown/uninstall 时删除端点并清理敏感运行状态。

## 安装与配对

安全本机自动写入不可避免地需要一次插件安装。发布物应包含 XPI；CLI 提供
`zot bridge setup/status`（最终命名在设计阶段确定），展示精确安装路径、检测
插件版本/协议和完成 token 配对。不能通过直接修改 Zotero profile 数据库来
“静默安装”。

MVP 的合理配对方式是插件生成本地 token，并通过明确的用户动作或受限配置文件
交给 CLI；实现期应在 Zotero 9 上验证最小交互。token 是本机桥接凭据，不是
Zotero Web API key。

## 用户决定

1. **已接受（2026-07-11）**：一次性安装最小 Zotero 插件，以获得 CLI/agent
   全自动本机写入；Run JavaScript 保留为诊断或应急 fallback。
2. **已决定（2026-07-11）**：分阶段交付。先交付 bridge + merge/dedupe tracer
   bullet + skill/docs/evals，再按独立子任务扩展 note/tag/collection/import/
   attachment/saved-search/status-sync 等操作族。
3. **已决定（2026-07-11）**：bridge setup 配对成功后将当前 profile 默认写后端
   设为 desktop；既有未 setup profile 继续 web。命令可显式覆盖，禁止任何
   desktop/web 自动 fallback。
4. **已决定（2026-07-11）**：批量 dedupe confirm 默认只执行 normal；low
   confidence 进入 skipped 列表，只有 include flag 或单组显式 merge 才放行。
