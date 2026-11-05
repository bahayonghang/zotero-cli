# Design — 本地整理能力完善

## Architecture

复用现有四条边界：SQLite 只读；LocalApiClient 负责授权本地写入；connector 只负责导入；ZoteroRemote 负责原有云端写入。禁止复用远端客户端加 localhost URL 来混合身份、版本或凭据。

Agent → 明确 item/collection key 与动作 → CLI 预览/计划 → 用户审阅 → 实例和版本校验 → Zotero 本地授权 → 条件提交 → 回读及结果报告。

A 扩展标签命令；B 扩展分类创建；C 增加显式计划文件驱动的本地整理入口。细节分别由子任务 design.md 定义，父任务不重复维护命令字段。

## Reuse boundaries

- 复用 LocalWriteArgs、local_credentials、LocalApiClient、SecretString、标准 envelope 与错误映射。
- 标签/成员纯计算可由单条与批量共用；只有实际出现第二个调用方时才提取共享函数。
- 保留 item dispatch/main 中既有 boxed future，防止 Windows 真实进程栈回归。
- 不提前引入通用 CRUD 框架、队列、策略引擎或新 crate。
- 本地标签使用 tags 完整对象集合；数组 PATCH 替换数组，不能仅提交新增元素。
- 分类创建是 POST + 对象级结果解析；与 item PATCH 的 204 回执不是同一个协议。

## Compatibility

现有 Web 默认值和单条 envelope API version=1 不变。新增命令、可选后端字段和报告是增量扩展；未知/回读失败仍不能凭 ok:true 判成功。批量命令自行定义严格退出状态并写进文档，不能顺便改变旧批量 Web 行为。

doctor 继续只读探测，不为展示新能力而授权/试写；按静态实现列能力时必须与 reachable、supported、verified 分开。不要借任务清除用户的旧配置或安装/卸载插件。

## Data and safety

- 所有写入由 Local API 取得实例、scope 和本地版本；本地读取快照仅可用于 Agent 发现候选。
- 普通条目与同库规则沿用基线。未知结果不自动重试；401 的明确拒绝允许现有一次重新授权。
- 新命令输入和计划是非可信数据，字段白名单解析；不执行计划中的脚本、URL 或路径。
- 文件化计划/报告含文献元数据，放用户明确指定的本地位置；测试数据放 ignored target/。不提交真实库内容或凭据。
- 不宣称用户使用的云端 Agent 模型也“离线”；这里只保证 Zotero 数据写入不依赖 Web API。

## Verification and recovery

单元/HTTP mock 覆盖协议和故障；CLI 进程测试覆盖解析、退出与 envelope；独立桌面库覆盖真实授权和 UI。
真实库验收前另行取得该测试操作授权并隔离 profile/data；创建任务不授权真实写入。

批量不承诺全有或全无。失败时保留已成功项、停止剩余项并输出证据。后续人工补偿只处理本任务修改过的字段，重新取版本并预览；不提供无条件恢复整个旧对象的机制。

## Planning review

本组设计已获实施批准，A/B/C 实现均在 dev 上继续验收。后续若范围、CLI 用户流程或风险策略实质变化，更新 PRD/design/implement 并重新审阅。本轮修复恢复现有设计的校验、停止和持久化契约，没有新增用户流程。
