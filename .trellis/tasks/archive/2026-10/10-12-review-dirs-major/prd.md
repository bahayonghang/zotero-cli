# 审查 dirs 6 到 7 的路径行为

## Goal

在 crate 锁升级之后，把本地 `dev` 的 `dirs` 从 6.0.0 升到 7.0.0，并关闭 PR #19。配置、状态和 PDF 缓存目录保持现有位置。

## Background

父任务：`10-12-dependabot-pr-review`。路径证据：`research/dirs-7-paths.md`。

dirs 7 的 `win.rs` 只把 `preference_dir` 从 LocalAppData 改为 RoamingAppData。`lin.rs` 与 `mac.rs` 相同。本仓库调用 `config_dir`、`home_dir` 和 `cache_dir`，没有 `preference_dir`。

本任务依赖 `10-12-review-crate-bumps` 先提交它的 `Cargo.lock`。

## Requirements

- R1：workspace 要求改为 `dirs = "7.0.0"`，锁文件解析到 7.0.0，`dirs-sys` 保持 0.5.0。
- R2：不新增 `preference_dir` 调用，不修改 `AGENTS.md` 的路径例子。
- R3：升级后 `rustls` 仍是 0.23.45。
- R4：本地提交完成后评论并关闭 #19。评论说明 Windows `preference_dir` 是唯一差异且本仓库未使用，并写明提交尚未推送。

## Acceptance Criteria

- [ ] `Cargo.toml` 的 dirs 要求是 `7.0.0`，锁文件是 dirs 7.0.0 与 dirs-sys 0.5.0。
- [ ] `cargo test -p zot-core` 与 `cargo test -p zot-local` 通过。
- [ ] #19 为 CLOSED 且未合并。

## Out of Scope

- 迁移用户已有配置。
- 更换路径库。
- 推送 `origin/dev`。
