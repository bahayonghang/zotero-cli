# 执行计划

1. 用户批准后只读复核最新 release、环境 deployment branch/tag policy 与 Cargo 版本；策略变化则先更新证据。
2. 新增 docs/agents/release.md，写 vX.Y.Z、Cargo X.Y.Z、发布权限与 Pages 验收边界。
3. README.md 加入口链接，人工核验示例；actionlint .github/workflows/ci.yml .github/workflows/deploy-docs.yml、git diff --check。
4. 强模型审查 AC1–AC3 的本地部分，记录 LOCAL PASS / HOSTED UNVERIFIED；不执行 tag/release/environment mutation。
5. 仅在未来明确发布授权后由发布工作流执行合规 tag 发布，读回对应 run 的 build/deploy 及 URL；成功后追加 hosted 证据，否则保留失败根因。

本子任务的文档交付与 hosted 故障闭环分栏；父任务不得把文档完成写成部署恢复。

## 精确执行文件清单
- docs/agents/release.md（新增）：vX.Y.Z / Cargo X.Y.Z、授权阶段、Pages 验收。
- README.md、README.zh-CN.md：同步发布契约入口及标签示例。
- Cargo.toml、.github/workflows/deploy-docs.yml、GitHub environment policy：只读对照，不改动；不增 tag 验证器。
