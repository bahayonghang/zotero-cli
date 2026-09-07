# 执行计划

1. 批准后复核 .github/workflows/ci.yml 和 deploy-docs.yml，确认无需重复修复当前已通过的 Rust jobs。
2. 增加 docs build job，明确 job-level `permissions: { contents: read }`，其余 token scope 不授予；沿用锁文件和 deployment Node 版本；保持 just ci 不变。人工检查权限键值与 absence of write scopes，不能只靠 actionlint。
3. 执行 actionlint .github/workflows/ci.yml .github/workflows/deploy-docs.yml。
4. 用户批准本子任务后即包含既有 lock 的依赖准备：若依赖缺失，执行 npm --prefix docs ci，再 npm --prefix docs run build；不新增/升级依赖或改变 lock。本轮规划不执行安装。
5. 在临时副本验证确定错误使 VitePress build 返回非零；确认真实文档和 lock 未改。
6. 更新 README.md 验证段与 C1 共享矩阵的检查边界；just ci、git diff --check。
7. 强模型审查 AC1–AC3；未获 remote delivery 授权时只交付本地结果，不擅自 push/开 PR/dispatch workflow。获授权后读取同 SHA hosted docs job 并补证。

依赖：C1 先建立共享 harnesses 文档再补验证段；C2 若同时修改 README 验证段应串行整合。C4 的 tag 约定与本任务代码独立。

## 精确执行文件清单
- .github/workflows/ci.yml：新增 docs job、显式权限和锁定安装/构建步骤。
- README.md、README.zh-CN.md：同步验证范围、命令和环境边界。
- docs/agents/harnesses.md：在 C1 创建后补五工具共同验证边界。
- docs/package-lock.json、docs/package.json、.github/workflows/deploy-docs.yml：只读参照，不改依赖或部署契约。
