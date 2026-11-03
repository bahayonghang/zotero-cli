# Evidence — 2026-11-02

## Source baseline

分析时仓库 dev，HEAD eb0345f，产品源码工作区 clean。前置本地内容实现为 16ce140，真实验收记录为 c9548ce。以下是读取现有源码得出的事实，不代表本次重新执行了旧验收：

- src/zot-cli/src/commands/item/tag.rs:26：add/remove 使用 ctx.remote；batch_update_tags 同样创建 WebBatchTagWriter。
- src/zot-cli/src/commands/collection.rs:10：create/rename/delete 走远端；add/remove 分 local；move 只接受 local。
- src/zot-cli/src/commands/local_write.rs：read_target、plan_fields、membership、execute 和 matches_patch 是本地写基线；tags 是保留字段。
- src/zot-desktop/src/local_api.rs:271：已有版本化 item PATCH、认证及实例校验；没有本地 collection POST/list snapshot 接口。
- src/zot-cli/src/cli/args.rs:532：WriteBackendArg 默认 Web；LocalWriteArgs 绑定实例/条目版本。
- src/zot-local/src/db.rs:1733：源库只读打开并通过 backup 建 snapshot，再只读访问。
- src/zot-cli/src/commands/mcp.rs：仍为 mcp-not-implemented，本组任务不依赖 MCP。
- .trellis/spec/zot-cli/backend/local-api.md：确认、错误和真实验收契约。
- justfile：just ci 为纯检查，技能生成需显式 skills-sync。

## Current runtime snapshot

本会话按未安装 zot 的 fallback，执行 cargo run -q -p zot-cli -- --json doctor：

- local_sqlite_read.available=true，db_exists=true。
- local_api_write.reachable=false，错误 local-api-unreachable；不能推断当前桌面版本不支持。
- local_http_read 和 connector 当前不可达。
- web_write.configured=false；PDF backend available=true。
- doctor 不试写，verified=false 不能当作实际写权限结论。
首次执行受工作区外 mbx 缓存权限限制，经允许重跑成功。没有修改主库、配置或执行授权弹窗。

## Historical desktop evidence

.trellis/tasks/archive/2026-10/10-13-zotero-local-content-operations/validation.md
及 acceptance/results.json 保存 Windows Zotero 10.0.6 隔离库的字段、作者、分类移动/增删、Deny/Allow/Always Allow、版本冲突与重启验收。
不涵盖本次新增本地标签/创建/批量能力，也不证明当前主库授权。真实组库与其他 OS 仍需区分证据级别。

## Official protocol sources

已在本次分析/规划中读取：

- https://www.zotero.org/support/dev/web_api/v3/local_api
- https://www.zotero.org/support/dev/web_api/v3/write_requests
- https://www.zotero.org/support/dev/client_coding/direct_sqlite_database_access

协议依据摘要：

- Zotero 10 本地 API 支持授权写入；实例 ID 与本地版本独立于 Web，写后可在 UI 看见。
- tags/collections 数组 PATCH 表示完整目标数组，必须保留非目标元素。
- collection POST 使用库版本条件或写 token，HTTP 成功仍须读取对象级结果。
- 本地写 token 在重启后丢失，不能据此承诺持久重试幂等性。
- SQLite 直接写绕过应用约束，保持只读。

这些协议事实支持设计；新增接口和失败策略仍须由 mock、真实 CLI 及隔离桌面库分层验证。

## Planning decisions and scope

用户要完善 Agent 本地整理，因此拆开单条标签、单条分类创建和批量整理，以先交付最直接缺口。
首版保留 Web 默认、显式 local、同库普通条目、用户授权和回读。分类删除/重命名、内置模型、自动回滚与通用恢复调度器不纳入。
新建批量入口是因为旧 tag batch 不能表达每篇不同标签和成员动作；不重复添加第二套检索器。
分类同级重名拒绝、批量首错停止和默认 50 上限属于本组待用户审阅的产品行为提案，尚不代表实施批准。

## Acceptance risks

1. 分类集合响应的本地库版本与 POST 返回结构要在目标版本验证；缺失保护时不能退化为无条件写入。
2. 一次性授权可能逐项弹窗；文档不能把用户批准 CLI 计划等同于 Zotero 授予长期权限。
3. 批量提交没有跨条目事务；报告必须保留 in-flight/unknown，不能用自动重试掩盖。
4. 计划和报告含文献信息；只保留必要字段，合成 fixtures 入库，真实结果留用户指定路径。
5. Agent 判错主题不由 HTTP 正确性解决；先审阅词表与理由、小规模验证，再扩大范围。
