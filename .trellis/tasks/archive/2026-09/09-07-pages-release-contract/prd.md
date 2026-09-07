# 对齐 release 标签与 Pages 部署契约

## Goal

统一 vX.Y.Z 发布约定与 Pages 保护策略，记录 hosted 验收边界；批准后实施

## Requirements

- R1：统一未来 release tag 与 github-pages 环境契约，推荐 vX.Y.Z；保留 Cargo 版本 X.Y.Z。失败 run 29644116256 使用 1.0.0，而环境仅放行 main branch 与 v* tag，见父任务 research/workflow-review.md。
- R2：项目发布说明明确本地准备、tag/release 发布、Pages hosted 验收是独立授权范围；本次批准文档改造不等于批准真实发布。
- R3：批准结论写入 README.md、README.zh-CN.md 与 docs/agents/release.md 并注明五套 harness 共用；不通过降低环境保护规则绕过契约。

## Acceptance Criteria

- [x] AC1（R1）：发布说明使用 v<workspace.package.version> 并说明 Cargo 数值版本不含 v；逐条对照当前 Pages main/v* policy。
- [x] AC2（R2）：在无发布授权时只完成本地准备，不改远端环境、不创建/删除/改写 tag、release；验收记录分别给出本地文档结果与 hosted UNVERIFIED。
- [x] AC3（R3）本地：说明进入共享入口并通过人工复核、actionlint 与 git diff --check。hosted 合规 tag 的真实 Deploy docs run 仍为 UNVERIFIED，未标 PASS。

本地：`docs/agents/release.md`、`README.md`、`README.zh-CN.md` 已写入；`actionlint` 与 `git diff --check` 通过；`Cargo.toml` 与 `deploy-docs.yml` 无 diff。hosted Deploy-docs / 合规 `vX.Y.Z` 发布路径仍为 UNVERIFIED。

## Authority

本轮只完成规划；用户批准后才实施。具体文件、验证机制与顺序见 design.md 和 implement.md；不得以创建本任务推断代码、安装或外部发布授权。
