# 审查四个 crate 小版本升级

## Goal

在本地 `dev` 上接受 PR #17、#18、#20、#21 的锁文件升级，然后关闭这四个 PR。不合并它们到 `main`。

## Background

父任务：`10-12-dependabot-pr-review`。证据：`../10-12-dependabot-pr-review/research/pr-triage.md`。

当前 `dev` 锁版本是 clap 4.6.1、thiserror 2.0.18、tar 0.4.45、http 1.4.0。workspace 的 caret 要求已经允许这些目标版本。本任务是四个子任务中的第一个，因为后面的 dirs 升级也要改同一份 `Cargo.lock`。

## Requirements

- R1：clap 升到 4.6.6。不改调用点，不主动升级 `clap_complete` 的要求。
- R2：thiserror 2.0.18 升到 2.0.20。锁里保留 thiserror 1.0.69。
- R3：tar 升到 0.4.46，并在 PR 评论中写明 GHSA-3cv2-h65g-fgmm。
- R4：传递依赖 http 升到 1.5.0。不把 workspace 里未引用的 `http = "1.3.1"` 改成直接依赖。
- R5：精确更新若被迫改变 `rustls` 或其他直接依赖版本，停止并保持原锁，不关闭对应 PR。
- R6：四个 PR 在本地提交完成后评论并关闭。评论写明本地提交哈希，以及该提交尚未推送。

## Acceptance Criteria

- [ ] `Cargo.lock` 含 clap 4.6.6、thiserror 2.0.20、thiserror 1.0.69、tar 0.4.46、http 1.5.0、rustls 0.23.45。
- [ ] `cargo check --workspace --locked` 通过。
- [ ] `cargo test -p zot-cli`、`cargo test -p zot-core`、`cargo test -p zot-local` 通过。
- [ ] #17、#18、#20、#21 为 CLOSED 且未合并，评论含本地提交哈希。

## Out of Scope

- dirs、GitHub Actions、MSRV workflow。
- 推送 `origin/dev`。
