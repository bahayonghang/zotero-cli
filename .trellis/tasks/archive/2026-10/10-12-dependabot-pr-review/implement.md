# 执行：父任务集成验收

四个子任务完成前不启动本任务，也不改产品文件。

## 顺序

1. `10-12-review-crate-bumps`
2. `10-12-review-dirs-major`
3. `10-12-review-actions-majors`
4. `10-12-reject-rust-toolchain-msrv`

## 验收

- [ ] `Cargo.lock` 含 clap 4.6.6、thiserror 2.0.20、thiserror 1.0.69、dirs 7.0.0、tar 0.4.46、http 1.5.0、rustls 0.23.45。
- [ ] `Cargo.toml` 的 `dirs` 要求是 `7.0.0`，`rust-version` 是 `1.85`，`http` 仍是未被成员引用的 `1.3.1`。
- [ ] checkout 与 setup-node 引用是 `@v7`；MSRV job 仍是 `dtolnay/rust-toolchain@1.85.0`；`node-version` 仍是 20。
- [ ] `dependabot.yml` 的 ignore 只包含 `dtolnay/rust-toolchain`。
- [ ] 在仓库根运行 `just ci`，退出码为 0。
- [ ] `gh pr view 17` 到 `24` 的状态都是 CLOSED，且都没有 MERGED。
- [ ] 把上列结果写入 `research/disposition.md`，包含每个本地提交哈希。

## 回滚点

集成验收失败时，按子任务提交从后往前 revert，不使用 `git reset` 丢掉已有的未推送历史。PR 保持关闭，直到用户另作决定。
