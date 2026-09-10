# 主任务执行顺序与审批包

## 当前阶段
四个子任务已按 C4→C1→C2→C3 完成本地实施、验证、提交并归档。
父任务 parent-check HEAD `f2946130cd4eadec5a45a06fbd0dd39356dbb3f5`。
未改产品文件。未 push、未 tag/release、未 dispatch、未改 GitHub environment。

本地发布门退出码：
- `actionlint .github/workflows/ci.yml .github/workflows/deploy-docs.yml`：0，0
- `npm --prefix docs run build`：0，0
- `git diff --check`：0，0
- `just ci`：0

确认：
- `.github/workflows/ci.yml` docs job `permissions` 仅为 `contents: read`
- `docs/package-lock.json`、`.github/workflows/deploy-docs.yml`、`Cargo.toml` 无未提交 diff
- hosted Deploy-docs / PR docs job / 五工具 fresh-start 保持 UNVERIFIED，不标 PASS

四个子任务已在 `.trellis/tasks/archive/2026-09/`，`status=completed`，`parent` 仍为 `09-07-evergreen-harness-audit`。`find_task_by_name` 只查活动任务目录，父任务归档不会把 `parent:null` 写到活动子任务。

父任务在本检查后提交规划产物再归档。

## 批准后
1. C4：固化 vX.Y.Z 发布契约，保留 main/v* 环境策略；先验收本地说明，hosted 实际发布另按授权执行。
2. C1：校正事实、CLAUDE 薄入口、五工具共享矩阵与 Grok/Kimi 手动步骤；不修改运行配置/探测算法。
3. C2：用回归先复现默认检查漏检，再扩大受管 skill 范围，保留纯检查。
4. C3：增加 PR docs build，不增加 deploy 权限；批准包括现有锁文件的 npm ci。
5. 每项由便宜模型执行时给精确文件/AC/允许命令，强模型复核；发现新权限或跨层语义问题退回审查。
6. 每项回写说明/skill 适用工具后做一次最终整合：just ci；actionlint 两工作流；npm --prefix docs run build；git diff --check。只重跑变化影响的检查，不无限扩测。
7. 逐工具记录 fresh-start 版本、规则来源、规划不实施、doctor 与镜像发现；未运行不填 PASS。读取同 SHA CI 结果；未经授权不 push/PR/dispatch。
8. 检查四子任务 AC 与父 AC，记录本地改造、外部发布、五工具运行三个独立状态；用户未授权外部动作时明确剩余证据，不宣称全面闭环。

## 本轮计划检查
- task.py validate 每个成员（仅验证结构，不能证明计划充分）。
- plan_precheck.py 主任务 --include-descendants（使用已安装 trellis-plan-review skill 的脚本）。
- 强模型独立审查主子任务全体，修正规划内冲突，不进入实施。
- git diff --check 与状态检查确认仅任务/审查材料新增。

## 回写拥有者
C1 → AGENTS.md / CLAUDE.md / docs/agents/harnesses.md / canonical skill 的必要事实。
C2 → mirror checker/test 与 README 验证段。
C3 → CI workflow 与验证说明。
C4 → docs/agents/release.md 与 README 发布入口。
适用工具必须写在共享说明中；团队知识库不是本轮新增目标，不另建重复知识副本。

