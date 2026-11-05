# 审查并处置 8 个 Dependabot PR

## Goal

把 Dependabot PR #17–#23 的等价升级写进本地 `dev`，拒绝 #24，并在 GitHub 上关闭这 8 个 PR。不把仍含 `rustls` 0.23.38 的 `main` 单独推进，也不抬高 Rust 1.85 MSRV。

## Background

证据在 `research/pr-triage.md`。`dev` 已把 `rustls` 升到 0.23.45。#22–#24 在 2026-10-01 的安全检查失败，扫到的是 `main` 上的 RUSTSEC-2026-0285。#17–#21 的全绿检查停在 2026-09-13，早于该公告。

2026-10-12 采用的落地方式：接受的改动进入本地 `dev`；不合并这 8 个 PR 到 `main`；#24 关闭；#17–#23 在对应提交出现在本地 `dev` 之后关闭。不推送 `origin/dev`，因为该分支还另有 6 个未推送提交。

## Requirements

| ID | 需求 | 任务 |
| --- | --- | --- |
| R1 | 八个 PR 都有书面处置和 GitHub 评论。#24 拒绝。#17、#18、#19、#20、#21、#22、#23 接受，版本见兼容性复核表。 | 四个子任务；父任务汇总 |
| R2 | MSRV job 保持 `dtolnay/rust-toolchain@1.85.0`。`Cargo.toml` 的 `rust-version` 保持 `1.85`。Dependabot 忽略 `dtolnay/rust-toolchain` 的全部引用，以免把编译器钉扎当成 action 版本。 | `10-12-reject-rust-toolchain-msrv` |
| R3 | `actions/checkout` 与 `actions/setup-node` 从 `@v4` 改为 `@v7`。`node-version` 仍是 20。不改其他 action。 | `10-12-review-actions-majors` |
| R4 | `dirs` 要求改为 `7.0.0`。不调用 `preference_dir`，不改 `AGENTS.md` 的路径例子。 | `10-12-review-dirs-major` |
| R5 | 锁文件升到 clap 4.6.6、thiserror 2.0.20、tar 0.4.46、http 1.5.0。保留传递的 thiserror 1.0.69。不把未使用的 workspace `http` 键改成直接依赖。 | `10-12-review-crate-bumps` |
| R6 | 全部改动在本地 `dev` 上串行完成。后一个子任务基于前一个子任务的 `Cargo.lock`。选定结果里的 `rustls` 保持 0.23.45。 | 子任务顺序见下；父任务验收 |
| R7 | 父任务核对四份子任务的结论、提交和 PR 关闭状态一致。 | 父任务 |

子任务顺序：`10-12-review-crate-bumps`，然后 `10-12-review-dirs-major`，然后 `10-12-review-actions-majors`，然后 `10-12-reject-rust-toolchain-msrv`。前两个都改 `Cargo.lock`，必须按这个顺序。父任务在四者完成前不启动。

## Acceptance Criteria

- AC1 / R1：八个 PR 均未合并进 `main`，各有一条评论；八个 PR 都已关闭。
- AC2 / R2：`ci.yml` 的 MSRV job 仍是 `@1.85.0`；`rust-version` 仍是 `1.85`；`dependabot.yml` 只忽略 `dtolnay/rust-toolchain`。
- AC3 / R3–R5：本地 `dev` 的锁文件和 workflow 含有上面列出的接受版本；`node-version` 仍是 20。
- AC4 / R6：`Cargo.lock` 里的 `rustls` 仍是 0.23.45。`just ci` 通过。
- AC5 / R7：父任务汇总写明每个 PR 的结论、本地提交和 GitHub 状态。

## Out of Scope

- 把 MSRV 抬到 1.85 以上。
- 升级这八个 PR 以外的依赖，或删除未使用的 workspace `http` 键。
- 推送 `origin/dev`、打开新的 GitHub PR、发布或打 tag。
- 修改 Zotero 库、凭据，或更换默认 Rust 工具链。
