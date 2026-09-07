# 常青项目审查与五套 harness 对齐

## Goal
基于当前 zotero-cli 结构、测试、失败工作流和五套 harness 的真实能力，提交可批准的最小改造计划。用户批准后按子任务执行并回写工具适用范围。

## Background
审查基线 dev / 5b53e7f5c2dc488160bd2ddf1ba614776403b1d0；开始时工作区干净。
本轮 just ci 通过，277 个不同 Rust 测试和 5 个 Python 测试无失败；当前 SHA hosted CI 全绿。
存在发布 tag 与 Pages 保护策略错配、项目说明事实/入口缺口、默认镜像检查漏掉第二 skill，以及 docs 只在发布时构建的预防缺口。
证据分别保存在 research/local-validation.md、research/workflow-review.md、research/harness-review.md；总览在 audit-report.md。历史失败不能冒充当前代码失败。

## Requirements
- R1：说明项目结构与关键调用/数据边界，检查五套 harness 的事实与规则一致性，区分已验证事实和未知。
- R2：记录现有检查的命令、结果、失败根因、当前与历史 SHA；不把缺环境误判为代码缺陷。
- R3：每个批准项有优先级、精确文件、最小设计、必须通过的验收与强/便宜模型分工。
- R4：建立主子任务、依赖顺序和最终整合标准，批准项回写到项目说明或 canonical skill 并注明适用工具。
- R5：本轮仅审查和规划；全部任务保持 planning。实施批准与外部发布/环境更改授权分开；不改用户全局配置或私有数据。

## Acceptance Criteria
- [x] AC1（R1）本地：报告覆盖 5 crate 的真实职责、入口和五工具规则/能力矩阵，说明来源。五工具 fresh-start / actual runtime 保持 UNVERIFIED，不标 PASS。
- [x] AC2（R2）本地：本地测试清单、历史 CI 根因与当前修复状态可追踪到命令/SHA/run；部署保护与构建失败分别归因。hosted 新 SHA CI 保持 UNVERIFIED。
- [x] AC3（R3）本地：四个子任务均有 prd/design/implement 和真实 context manifests；每项 AC 可追踪到文件/机制/检查，不含模板 seed。四子任务已归档，`parent` 仍指向本父任务。
- [x] AC4（R4）本地：C4→C1→C2→C3 已实施、提交并归档。parent-check HEAD `f2946130cd4eadec5a45a06fbd0dd39356dbb3f5`：`actionlint` 0/0，`npm --prefix docs run build` 0/0，`git diff --check` 0/0，`just ci` 0。docs job 权限仅 `contents: read`。`docs/package-lock.json`、`deploy-docs.yml`、`Cargo.toml` 无未提交 diff。hosted Deploy-docs / PR docs job / 五工具 fresh-start UNVERIFIED，不标 PASS。
- [x] AC5（R5）本地：本检查未改产品文件；未 push、未 tag/release、未 dispatch、未改 GitHub environment。子任务已归档后再归档父任务，避免把 `parent:null` 写到活动子任务。hosted Deploy-docs、hosted PR docs job、五工具 fresh-start 保持 UNVERIFIED，不标 PASS。

## Child Task Map
| ID | Task | Priority | Requirement ownership |
| --- | --- | --- | --- |
| C1 | 09-07-harness-rules-alignment | P1 | 共享事实、原生入口、Trellis 手动路径与知识回写 |
| C2 | 09-07-skill-mirror-coverage | P2 | 两个 skill 的默认门禁范围与回归 |
| C3 | 09-07-evergreen-ci-validation | P2 | docs PR 构建门禁和验证边界 |
| C4 | 09-07-pages-release-contract | P1 | vX.Y.Z 发布与 Pages 保护契约 |

## Out of Scope
本轮不修改代码或正式规则，不安装依赖，不触发 workflow，不提交/push/开 issue/PR，不创建/改写 release/tag，不改变 Pages 环境，不新建 MCP 服务或全平台适配框架，不把 Codex 原生记忆复制到团队知识库。

## Approval
推荐批准 C4 → C1 → C2 → C3 的本地实施与验证。批准 C3 包含按现有 lock 执行 npm ci 的环境准备，不新增/升级依赖。真实 tag/release 发布与五工具不可用环境准备仍需相应明确授权或可用条件；未获授权时可以完成本地改造，但不得声称这些外部验收已通过。
