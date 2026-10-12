# 执行：关闭 MSRV 误升级

工作分支：本地 `dev`。在 Actions 子任务提交之后进行。

## 清单

- [ ] 在 `.github/dependabot.yml` 的 github-actions 条目加入只针对 `dtolnay/rust-toolchain` 的 ignore。
- [ ] 确认 cargo 与 npm 条目没有 ignore，checkout 与 setup-node 也没有被忽略。
- [ ] 确认 `ci.yml` 仍有 `dtolnay/rust-toolchain@1.85.0`、`@stable` 和 nightly 引用。
- [ ] 提交，只包含 `dependabot.yml`。
- [ ] 评论并关闭 #24。评论说明 MSRV 钉扎、rustup 404，以及 ignore 的范围。不把 #24 的 diff 应用到本地。

## 验证

diff 只含 `dependabot.yml`。不运行 `just ci`；父任务统一跑。

## 回滚点

`git revert` 该提交。#24 不重开。
