# 处置 rust-toolchain MSRV 误升级

## Goal

保持 Rust 1.85 MSRV 检查，关闭 PR #24，并让 Dependabot 不再把 `dtolnay/rust-toolchain` 的引用当成可升级版本。

## Background

父任务：`10-12-dependabot-pr-review`。证据：`../10-12-dependabot-pr-review/research/pr-triage.md`。

#24 只把 MSRV job 的 `dtolnay/rust-toolchain@1.85.0` 改成 `@1.120.0`。2026-10-01 安装失败，rustup 对 `1.120.0` 返回 404。`Cargo.toml` 的 `rust-version = "1.85"` 是仓库声明的最低支持版本。`@stable` 和 `@nightly` 也是工具链选择器。

本任务在 Actions 子任务之后改 `.github/dependabot.yml`，避免和 workflow 引用升级编进同一次提交。它不修改 `ci.yml`。

## Requirements

- R1：不合并 #24，不改变 `@1.85.0`、`@stable`、`@nightly`。
- R2：在 `github-actions` 更新里忽略 `dtolnay/rust-toolchain`。忽略不能覆盖 `actions/checkout` 或 `actions/setup-node`。
- R3：评论并关闭 #24。评论说明这是 MSRV 编译器钉扎、1.120.0 安装得到 404，以及 ignore 会阻止同类 PR。

## Acceptance Criteria

- [ ] `ci.yml` 的 MSRV job 仍使用 `dtolnay/rust-toolchain@1.85.0`。
- [ ] `Cargo.toml` 的 `rust-version` 仍是 `1.85`。
- [ ] `dependabot.yml` 含且只含这一个新的 toolchain ignore。
- [ ] #24 为 CLOSED 且未合并。

## Out of Scope

- 升级 stable 或 nightly。
- 处理 #24 上由 `main` 的 rustls 引起的安全检查失败。
- 推送 `origin/dev`。
