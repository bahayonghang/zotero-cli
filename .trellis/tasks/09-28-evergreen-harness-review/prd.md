# 常青项目复审与五套 harness 能力边界

## Goal
以当前证据形成最小改造包，使 Claude Code、Codex、Grok Build、Kimi Code、Oh My Pi（OMP）遵守一致的项目契约。强模型负责规划、根因判断和验收；低成本模型只执行批准后的确定性工作。

## Background
审查日期 2026-09-28；基线 dev / ba4e10e987e7afe65dccb8450445d41bc9e08d79；初始 clean。5 个 Rust crate，edition 2024，MSRV 1.85，版本 1.0.1。权威入口为 Cargo.toml、justfile、src/zot-cli/src/main.rs。旧审查已归档，本轮不重复实施已修复事项。完整证据见 audit-report.md 和 research/。

## Requirements
| ID | 需求与依据 | 拥有任务 |
| --- | --- | --- |
| R1 | 修复 Cargo.lock:1558-1570 的 rustls 0.23.38；audit/deny 均命中 RUSTSEC-2026-0285 | C1 security-lock-refresh |
| R2 | 修复 workflow_phase.py:135-176 的平台别名匹配，claude/kimi/omp 不再只输出标题 | C2 harness-platform-routing |
| R3 | 对齐 AGENTS.md:90-95、harnesses.md:30-36 与实际 config.yaml:129-145 / workflow.md:475-526；标明五工具规则、模型和权限边界 | C3 harness-contract-refresh |
| R4 | 更正双语 README:15 的 Desktop 合并说法，遵从 item/merge.rs:125-126；区分受管镜像与真实客户端发现 | C3 harness-contract-refresh |
| R5 | 补足安全结果失效后的检测空档；ci.yml:3-8 无 schedule，9 月 10 日全绿早于 9 月 14 日公告 | C4 evergreen-security-cadence |
| R6 | 所有结论绑定命令、退出码、SHA、日期与未验证范围 | 父任务集成验收 |
| R7 | 批准且验收通过的结论回写项目说明/规范，注明适用工具 | C2/C3/C4 + 父任务 |

## Acceptance Criteria
- AC1 / R1：C1 的安全、locked/MSRV/依赖检查通过，无 advisory ignore 或广泛升级。
- AC2 / R2：五平台有效步骤、别名、Codex 模式回归通过，测试能检出修复前空正文。
- AC3 / R3-R4：矩阵与真实配置一致；CLAUDE.md 保持薄入口；无 Desktop 合并承诺。
- AC4 / R5：schedule 只运行 security，原有事件检查不减少；actionlint 通过；真实 scheduled run 未发生前保持 UNVERIFIED。
- AC5 / R6：分开当前失败、已修复历史失败、环境限制和原因未查明项。
- AC6 / R7：有适用工具标识的写回完成，强模型审查最终 diff。
- AC7：规划阶段只新增本任务树，主任务和 4 个子任务在批准前保持 planning；批准后的任务状态按实际进展更新，关系与链接有效。

## Scope and Authority
规划阶段授权读取、现有测试、诊断、只读 GitHub 记录及规划证据。用户于 2026-09-28 明确回复“批准执行”，批准 C1–C4 的本地实施及必要验证。C1/C2 可独立推进，C3 依赖 C2，C4 在 C3 后编辑共享说明。允许按进展运行 task.py start。

拟议本地实施包含精确依赖修补、必要的固定版本验证工具准备（不改全局默认工具链）、局部 Trellis 修复、说明和 CI 调整。文件/验收见子任务。

不在范围：Zotero 库写入、凭据或全局 harness 设置、五客户端付费模型会话、Trellis 上游/npm 缓存、提交推送/PR/Issue/标签/发布/环境策略、Basic Memory 写入。

## Approved Decisions (2026-09-28)
- C1 优先只更新锁文件至最小兼容 rustls 修补版本。
- C2 在平台匹配边界归一化别名，保留一份工作流。
- C3 显式设置项目 Codex inline，落实项目已声明的约定；当前实际 auto 与拟议设置须分开记录。其他平台保留实际 pull/hook 路径，分派仍服从当次授权。
- C4 推荐每周一 08:17 UTC，仅运行现有 security job。
- 写回选择仓库 docs/agents 和 .trellis/spec，不新建通用 skill，不写团队知识库。

## Deferred Evidence
五客户端新会话、真实 Web/connector mutation、PDF/embedding 工作流、修补后的 hosted gate 均未验证，保留在本地实施授权之外。
