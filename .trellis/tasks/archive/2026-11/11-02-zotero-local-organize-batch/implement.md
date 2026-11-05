# Implementation plan — 批量整理

## Preconditions

- [x] 用户批准本设计；A 的自动化检查已通过，B 的分类创建命令已实现。隔离桌面核心验收已授权并通过。
- [x] 读取父任务及 A/B artifacts、local-api spec、JSON/错误/日志 specs。
- [x] 核对现有 CommandOutput/退出机制，确定支持带报告的非零结果；不重构无关命令。
- [x] A/B 用户流程与已审阅 design 一致，未改本设计，未停下来等待重新审阅。

## Steps

1. [x] 定义 request/plan/report v1 及字段白名单，添加计划解析和纯计算测试。
   验证混合动作冲突、重复 key、空输入、非法库、超限、未知字段；不同条目不同决策。
2. [x] 实现 plan：本地 API 读取、完整目标集合、每项版本、before/after、分类身份。
   验证不读凭据、不授权、不 PATCH；不靠 SQLite；输出完整列表而非仅 sample。
3. [x] 实现保存/加载和确切字节 hash；apply 绑定 scope/profile/server、数量和版本。
   验证文件改动/无效 schema/任意 patch 注入/目标漂移均在任何写入前拒绝。
4. [x] 实现串行执行及每条一次 PATCH，复用 A 和成员逻辑。
   已验证三条不同决策成功、51 条显式提高上限执行、第二条未知后停止、no-op 版本漂移、首项失败、401 与回读失败；见 validation.md。
5. [x] 实现报告提交前/后持久化及非零 JSON 契约。
   已验证不覆盖已有文件、提交前/后真实文件系统失败，以及强制终止测试子进程后磁盘 in-flight。生产 CLI 隔离库成功路径已验证单 envelope、退出 0、3 applied/verified 及磁盘报告与 stdout 一致；非成功退出由自动化验证。
6. [x] 增加 library organize clap/help/真实进程测试；保持旧 Web batch 行为。
7. [x] 更新双语 CLI/工作流、canonical zot skill/evals、local-api spec。
   指导 Agent 使用有限词表、明确 key、完整预览、小范围试运行与不确定结果停止。
8. [x] 隔离库跑 3-5 条不同决策的真实闭环；API、UI、重启和无关数据不变验收。已完成，见父 validation.md。
9. [x] 核对父任务所有 AC，记录未覆盖环境，不把 mock 故障标为真实故障验证。

## Checks

- cargo test -p zot-cli --locked
- cargo test -p zot-desktop --locked
- just ci
- npm --prefix docs run build
- git diff --check

按实际模块命名追加 focused tests；不重复完整测试，除非代码变更或失败需要。有意同步 skill 镜像后运行纯检查。

## Completion / rollback

validation.md 对应 B-AC1-B-AC8，保存合成请求/计划/报告 fixture，秘密和真实库数据不得入库。
出现报告不完整、越界 mutation、错误实例或盲重试问题时停止推广；只修复本子任务。删除代码不能撤销库写入，不提供虚假的“整批回滚”按钮。
