# Design — Agent 批量整理

## Why a separate plan entrypoint

旧 item tag batch 是“同一组筛选结果应用同一标签操作”；本任务需要“一篇一策”并同时表达成员变化。首版不改变旧命令、不新建顶层命令，提议在 library 下增加 organize plan/apply。
不添加同义入口，也不把模糊 query 作为 apply 的输入。Agent 先用现有读取能力确定 keys，再提交显式决策。

## Proposed CLI

以下是待实现接口：

- library organize plan --file requests.json --output plan.json --write-backend local [--max-affected 50]
- library organize apply --plan plan.json --confirm --expected-plan-sha256 HASH --report report.json --write-backend local [--max-affected 50]

现有 global --json/--library/--profile 继续生效。此新入口只接受 local；apply 无 confirm 拒绝并提示先审阅 plan。plan/report 不覆盖已有文件，使用新的用户指定路径。apply 的 scope/profile 必须匹配计划；不会默认切换实例或 profile。

## Request v1

顶层为 schema_version=1 与 items 数组。每项：

- item_key；
- add_tags/remove_tags：可选字符串数组；
- add_collections/remove_collections：可选已有 key 数组；
- move_collection：可选 {from,to}；
- reason：可选说明文字，仅展示给用户，不参与执行逻辑。

至少有一个动作。未知字段、重复 item_key、同名 add/remove、同 key add/remove、move 与其他成员动作同时出现均拒绝；避免隐含操作顺序。库由 CLI 指定，一份请求只能针对一库。字段/作者/创建分类不是允许动作。
Agent 给出的 reason 不是可信指令，不用于运行脚本、访问 URL 或读其他路径。

## Bound plan

planner 从 Local API 获取实际条目和分类，将请求转换为每条最多一个 tags/collections PATCH：

- schema_version、backend=local、server_id、library、有效 profile 身份；
- max_affected、目标总数、动作数、实际变化条目数、无变化条目数；
- 按输入顺序的 item_key、title、expected_version、original_request；
- before/after（只包括本次动作涉及的 tags/collections）、分类 key/name/version。

保留无变化项，使报告完整。分类目标版本变化也导致重新预览，避免用户按名称审阅后名称已改变却照常执行。apply 不重新搜索候选。

输出计划文件使用固定 UTF-8 序列化，SHA-256 针对确切文件字节；hash 在命令 JSON 输出中，不自嵌入文件。确认 hash 不匹配即拒绝。hash 只证明执行的是用户审阅的文件版本，不是签名、权限或授权凭据。
apply 仍严格检查 schema/字段白名单，并重算 original_request 对 before 的结果，确认与 after 一致；不能把任意 after JSON 当作可提交 patch。

## Preflight / execution

1. 在授权前验证计划格式、hash、backend/profile/scope、实例、数量上限以及全部目标状态和版本。任何错误使整个批次零写入。
2. 必须建立新的可写执行报告；无法落盘则不开始远端 mutation。
3. 按计划顺序串行处理。每项提交前重新检查条目/分类身份与版本；无变化项也不得忽略已改变的版本。
4. 同一条目的 tags 与 collections 合成一个条件 PATCH。重复用 A/既有成员算法，不分别发多次 PATCH 造成半条目状态。
5. 沿用凭据缓存与一次性授权。一次性授权可能要求后续条目再次授权；不擅自选择 Always Allow，不承诺一次授权即可整个批次。
6. 每项可靠回执和回读一致才 applied。冲突、拒绝、unknown 或 verification-failed 立即停止后续条目，写入 not-attempted 结果。
7. 仅对明确 401 拒绝复用现有一次重新授权；不因网络错误、5xx 或结果未知重发。

预检不是跨条目事务：其他程序仍可能在预检后修改，靠每项条件写入保护。计划不承诺执行顺序之间跨对象原子性。

## Execution report / exit

报告含 plan_sha256、server/library、时间、总计数与全部逐项结果；每项含 before/after、expected_version、observed_version、write_accepted、verified、error。
聚合 state：

- applied：至少一项 changed 且所有 changed 项 verified；
- unchanged：所有项无变化且未提交；
- partial：有已验证成功项，同时存在失败、未知或未执行项；
- failed：无已验证成功且至少一项明确失败；
- unknown：无已验证成功且至少一项提交未知或已接受但未验证。
逐项保留细分状态，不靠聚合隐藏 verification-failed。计数必须覆盖完整目标集合。

只在所有项 applied/unchanged 时退出 0；其余退出非 0，stdout 仍保持一个标准 JSON envelope，data 带完整报告。新增命令应使用现有输出/退出扩展点，必要时增加可携带报告的错误，而不改变旧命令全局语义；实际映射由 CLI 契约测试固定。

## Durable evidence / interruptions

执行报告以原子替换方式更新，每个提交前先落盘 in-flight，回读后落盘最终状态。文件首次创建不覆盖现有用户文件。

- 初始/提交前落盘失败：当前项不提交，停止后续项。
- 提交后落盘失败：stdout 尽可能输出已知结果并非零退出；磁盘 in-flight 项视为待核查，不重发。
- 进程被强制中止：in-flight 不等于 failed/成功；下次人工读取 API 复核。
报告不用于自动 resume/replay；文件只记录必要字段、绝不保存 authorization key。文件路径来自明确 CLI 参数，不读取计划中伪造的路径字段。

## Recovery and limits

重放旧计划需再次通过全部版本条件，不能跳过冲突当作成功。修复/补偿应生成新请求与新预览；不恢复整个旧对象，避免覆盖后续用户编辑。
默认上限 50 按实际发生变化的条目计数，同一条目多个动作只计一次；它是用户风险界限，不是 HTTP bulk 上限。plan/apply 都校验；需要扩大时两个阶段显式采用相同上限且重新审阅。串行执行不提供吞吐量/全库耗时保证。

## Source scope

主要落点：CLI args、commands/library 路由及新的 organize 模块、A/现有 local_write 的必要共享函数、对应 inline/JSON process tests；仅为精确报告回读版本按需扩展 desktop client。
新增版本化计划/report 模型放 CLI 编排层，不为了尚不存在的第二消费者迁移整个 zot-core。文档含完整合成请求/计划/部分失败示例与 Agent 操作指南。
