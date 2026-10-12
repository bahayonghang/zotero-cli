# 执行：四个 crate 锁升级

工作分支：本地 `dev`。先于 dirs 子任务。

## 清单

- [ ] 确认 `rustls` 当前是 0.23.45，thiserror 1.0.69 仍在锁里。
- [ ] 依次运行设计文档中的四条 `cargo update`。某条失败就停，不改用无版本约束的 `cargo update`。
- [ ] 阅读 `git diff -- Cargo.lock`。列出 clap 相关传递 crate 的版本变化。
- [ ] 若 `rustls` 不是 0.23.45，或 thiserror 1.0.69 消失，或任何 workspace 直接依赖的版本被改动，恢复锁文件并停止。
- [ ] `cargo check --workspace --locked`
- [ ] `cargo test -p zot-cli`
- [ ] `cargo test -p zot-core`
- [ ] `cargo test -p zot-local`
- [ ] 提交，只包含 `Cargo.lock`。
- [ ] 用提交哈希评论并关闭 #17、#18、#20、#21。#20 的评论写明 GHSA-3cv2-h65g-fgmm。四条评论都写明提交还在本地 `dev`，尚未推送，也不会合并到 `main`。

## 验证

上面的 check 与三个 crate 测试是本任务门禁。全仓库 `just ci` 留到父任务集成验收。

## 回滚点

`git revert` 该锁文件提交。已关闭的 PR 不自动重开。
