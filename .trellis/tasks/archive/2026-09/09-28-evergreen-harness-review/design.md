# 设计与责任边界

| 层 | 权威对象 | 改动 | 验证 |
| --- | --- | --- | --- |
| 依赖安全 | Cargo.lock + RustSec | 最小补丁 | audit/deny、locked、MSRV、udeps |
| 平台上下文 | workflow_phase.py + workflow.md | 别名归一化 | 五平台真实 CLI 渲染 |
| 执行策略 | 显式 config + AGENTS + harnesses.md | 配置/说明对齐 | 模式解析、文档复核 |
| 用户能力说明 | WebMergeWriter + canonical skill | 双语首屏修正 | 后端与表述核对 |
| 定期安全检查 | CI workflow | 周度安全触发 | 事件/job 矩阵、actionlint |

## Task Map
- C1 [security-lock-refresh](../09-28-security-lock-refresh/prd.md)，P1，独立。
- C2 [harness-platform-routing](../09-28-harness-platform-routing/prd.md)，P1，独立。
- C3 [harness-contract-refresh](../09-28-harness-contract-refresh/prd.md)，P1，依赖 C2。
- C4 [evergreen-security-cadence](../09-28-evergreen-security-cadence/prd.md)，P2，依赖 C1；与 C3 共享说明文件，串行执行。
父任务负责需求映射、批准记录和集成验收，不作为产品实施目标。

## Design Choices
依赖修补不更换 TLS 后端或关闭校验。平台匹配归一化连接 cli_adapter 的平台 ID 和工作流显示名；保留 Codex 虚拟平台与显式模式。

当前 Trellis 0.7.0-beta.3。修复只落本仓库 .trellis/scripts，不修改上游或全局安装。不改 .template-hashes.json 掩盖本地补丁；回归和说明作为未来 trellis update 后复核点。

CLAUDE.md 继续只导入 AGENTS.md。五工具不复制完整规则。两套受管 skill 镜像一致，不代表五客户端实际发现、hook 或子代理执行通过。

强模型决定范围、兼容性、权限与最终验收。低成本执行者只接冻结的文件、AC、命令和失败升级条件；不按 harness 品牌推断价格。

## Writeback
- C2/C3：docs/agents/harnesses.md 记录五平台 ID、能力、模型交接和加载验收；AGENTS.md 保留共享短约定。
- C3：.trellis/config.yaml 的 inline 仅适用 Codex；双语 README 的后端事实适用全部工具。
- C4：docs/agents/harnesses.md 与 .trellis/spec/zot-cli/backend/quality-guidelines.md 记录五工具共用的安全门和证据时效。
- C1：验证证据落父任务 research，由 C4 提炼长期规范。
仅把已批准且经验证的结论写入正式说明；本次不改 canonical skills。

## Rollback
按子任务隔离 diff。C1 恢复旧锁会恢复风险，不能当可发布状态。C2/C3 失败停止集成。C4 可回滚新增 schedule 和对应条件，不移除原有 security gate。
