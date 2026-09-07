# 固化持续集成与文档验证边界

## Goal

基于本地和 GitHub 证据处理现存工作流缺口，建立准确验收；批准后实施

## Requirements

- R1：在普通 PR 中构建中英文 docs，避免只在发布时才发现文档编译错误；现有 .github/workflows/ci.yml 没有 docs build，deploy-docs.yml:7 仅 release/manual。
- R2：沿用 docs/package-lock.json 和现有构建命令，不混入发布权限；本地 Rust just ci 保持纯检查。
- R3：准确记录 current-SHA、历史失败、环境缺失与 hosted 证据；不重新修复已在 5b53e7f 解决的镜像未安装和 nightly action ref。
- R4：验证边界回写 README.md、README.zh-CN.md 与 docs/agents/harnesses.md，适用于五套工具。

## Acceptance Criteria

- [x] AC1（R1）本地：PR CI 存在独立 docs job，执行 npm ci 和 npm run build；临时副本坏链接/注入错误使 build 非零；不部署 Pages。hosted docs job UNVERIFIED。
- [x] AC2（R2）本地：复用 lock 和 Node 20；job-level permissions 仅 contents: read；lock 与 deploy-docs.yml 无 diff；actionlint 通过。
- [x] AC3（R3、R4）本地：just ci 通过且不跑 VitePress；README 与 harnesses 写明五工具验证边界。hosted PR/push docs job UNVERIFIED。

## Authority

本轮只完成规划；用户批准后才实施。具体文件、验证机制与顺序见 design.md 和 implement.md；不得以创建本任务推断代码、安装或外部发布授权。
