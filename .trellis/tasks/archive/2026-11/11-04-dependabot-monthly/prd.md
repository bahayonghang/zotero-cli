# 处置 Dependabot PR 并改为每月检查

## Goal

把能合进 `main` 的 Dependabot 补丁合并进去，关掉过不了 Rust 1.85 的 PR，删掉这 5 个远程分支，并让默认分支上的 Dependabot 改为每月检查一次。

## Background

默认分支是 `main`。2026-11-04 复核时 `origin/main` 为 `0e6d584`（Merge pull request #16）。`origin/dev` 为 `9635858`。`dev` 有 30 个提交不在 `main` 里，`main` 有 4 个合并提交不在 `dev` 里。`5b368d0..origin/main` 没有改 `Cargo.lock`。

这 5 个 PR 的 base 已是 `0e6d584`，作者都是 Dependabot，状态都是可合并但检查不稳定。`main` 没有分支保护，也没有 ruleset。仓库允许 merge、squash、rebase。`deleteBranchOnMerge` 为 false。历史合并使用 merge commit。

`main` 的锁版本：chrono 0.4.44、clap 4.6.1、flate2 1.1.9、uuid 1.23.1、quick-xml 0.41.0、rustls 0.23.38。`dev` 的 clap 已是 4.6.6、rustls 已是 0.23.45。用户已选定：三个补丁只进入 `main`，`dev` 不带走这些锁更新。

证据和 CI 日志在 `research/pr-triage.md`。

## Requirements

| ID | 需求 |
| --- | --- |
| R1 | 用 merge commit 把 #25、#26、#27 按这个顺序合并进 `main`。#25 将 chrono 锁到 0.4.45，#26 将 flate2 锁到 1.1.10，#27 将 clap 锁到 4.6.7。三者都只改 `Cargo.lock`。 |
| R2 | 关闭 #28 和 #29，不合并。#28 的 uuid 1.28.0 要求 rustc 1.89.0。#29 的 quick-xml 0.42.0 要求 rustc 1.86，并让 `src/zot-remote/src/oa.rs` 编译失败，其中包括约 312–318 行的字节串比较和 330、338、348 行的 `decode()`。每条关闭都留下原因评论。 |
| R3 | 删除 5 个远程分支：`dependabot/cargo/chrono-0.4.45`、`dependabot/cargo/flate2-1.1.10`、`dependabot/cargo/clap-4.6.7`、`dependabot/cargo/uuid-1.28.0`、`dependabot/cargo/quick-xml-0.42.0`。 |
| R4 | 在 `main` 的 `.github/dependabot.yml` 把 cargo 和 `/docs` npm 的 `interval` 改为 `monthly`。github-actions 保持 `monthly`。保留两边现有的 `open-pull-requests-limit` 和 labels。cargo 忽略 uuid `>= 1.27.0` 与 quick-xml `>= 0.42.0`。github-actions 忽略 `dtolnay/rust-toolchain`，与 `.trellis/spec/zot-cli/backend/quality-guidelines.md` 第 98–99 行的契约一致。这个文件改动提交并推送到 `origin/main`。 |
| R5 | `dev` 的 `Cargo.lock` 和 `Cargo.toml` 保持不动，不推送 `origin/dev`。 |
| R6 | 不修改 Rust 1.85、`rust-version`、audit 或 deny 配置，也不用 `continue-on-error` 或 advisory ignore 把安全检查改成通过。 |

## Acceptance Criteria

- AC1 / R1：#25、#26、#27 是已合并状态。`origin/main` 的 `Cargo.lock` 含 chrono 0.4.45、flate2 1.1.10、clap 4.6.7。
- AC2 / R2：#28、#29 已关闭且没有 merged。各自有一条说明 MSRV 的评论。`origin/main` 的 uuid 仍是 1.23.1，quick-xml 仍是 0.41.0。
- AC3 / R3：`git ls-remote --heads origin dependabot/*` 没有结果。
- AC4 / R4：`origin/main` 的 `dependabot.yml` 里 cargo 与 npm 为 `monthly`，actions 仍为 `monthly`，并含 R4 的三条 ignore。
- AC5 / R5：`origin/dev` 仍是 `9635858`。本地 `dev` 的锁文件没有这三个补丁。
- AC6 / R6：diff 不包含 `ci.yml`、`Cargo.toml` 的 `rust-version`，以及 audit ignore。

## Out of Scope

- 把 chrono、flate2、clap 的锁更新写进 `dev`。
- 把 uuid 升到仍支持 Rust 1.85 的 1.26.0，或改 quick-xml 调用去适配 0.42。
- 处理 `main` 上已有的 rustls 0.23.38、quinn-proto、webpki 漏洞。当前 `dev` 上另行存在的 11 个 `cargo audit` 漏洞也不在本任务。
- 推送 `origin/dev`、发布或打 tag。
- 指定 Dependabot 的具体时刻。`monthly` 按 GitHub 的定义在每月 1 日运行，`day` 只对 weekly 有效。
