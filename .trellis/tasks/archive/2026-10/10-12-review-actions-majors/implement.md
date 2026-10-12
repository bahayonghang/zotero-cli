# 执行：Actions v7

工作分支：本地 `dev`。在 dirs 子任务提交之后进行，使这次提交只含 workflow。

## 清单

- [ ] 把两份 workflow 里的 `actions/checkout@v4` 全部改为 `@v7`。
- [ ] 把两份 workflow 里的 `actions/setup-node@v4` 改为 `@v7`。
- [ ] 核对 `node-version: 20`、`fetch-depth: 0` 和 MSRV 的 `@1.85.0` 仍在。
- [ ] `git diff` 只包含这两个文件中的上述引用。
- [ ] 提交。
- [ ] 评论并关闭 #22 与 #23。评论写明本地提交哈希尚未推送，并且不合并原 PR 到 `main`。

## 验证

本地门禁是 diff 范围。hosted job 的通过记录已经在 2026-10-01 的 PR #22 与 #23 上。本任务不推送，因此不会产生新的 GitHub Actions 运行。父任务的 `just ci` 不覆盖 action 运行时。

## 回滚点

`git revert` 该提交。
