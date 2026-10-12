# 审查 Actions 大版本升级

## Goal

在本地 `dev` 上把 `actions/checkout` 和 `actions/setup-node` 从 `@v4` 升到 `@v7`，并关闭 PR #22 与 #23。

## Background

父任务：`10-12-dependabot-pr-review`。证据：`../10-12-dependabot-pr-review/research/pr-triage.md`。

两个 workflow 的现有输入是 checkout 的 `fetch-depth: 0`，以及 setup-node 的 `node-version: 20`、`cache: npm`、`cache-dependency-path: docs/package-lock.json`。`docs/package.json` 没有 `packageManager`。仓库没有 `pull_request_target` 或 `workflow_run`。2026-10-01 的功能 job 和 docs job 在 v7 上已经通过；当时的安全失败来自 `main` 的 rustls，不是这两条 diff。

本任务不改 `Cargo.lock`。它可以在 dirs 子任务之后进行，以避免同时编辑时把 workflow diff 和锁 diff 混在一次提交里。

## Requirements

- R1：`ci.yml` 的六处 checkout 和 `deploy-docs.yml` 的一处 checkout 改为 `@v7`。
- R2：两个文件里的 setup-node 改为 `@v7`。`node-version` 保持 20。
- R3：不改 `dtolnay/rust-toolchain`、`taiki-e/install-action`、`Swatinem/rust-cache`，以及 Pages 相关 action。
- R4：本地提交完成后评论并关闭 #22 与 #23。评论说明不合并到 `main`，以及提交尚未推送。

## Acceptance Criteria

- [ ] 两个 workflow 中不再有 `actions/checkout@v4` 或 `actions/setup-node@v4`。
- [ ] `node-version: 20` 仍在两处 setup-node 上。
- [ ] MSRV job 仍是 `dtolnay/rust-toolchain@1.85.0`。
- [ ] #22 与 #23 为 CLOSED 且未合并。

## Out of Scope

- 运行或等待新的 GitHub Actions。本地不能执行 hosted runner。
- 推送 `origin/dev`。
