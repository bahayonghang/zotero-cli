# 处置 Dependabot PR #35–#39 并清理分支

## Goal

把仍开着的 5 个 Dependabot PR 处置完。`main` 收下能通过 Rust 1.85 的三个更新，关掉两个不兼容更新，并删掉它们留下的远端分支和本克隆里的跟踪引用。

## Background

仓库是 `bahayonghang/zotero-cli`。这 5 个 PR 是 #35、#36、#37、#38、#39，对应截图里的 5 个 `dependabot/cargo/*` 分支。`main` 和 `dev` 也出现在分支列表里，但它们不是这 5 个 PR 的头。版本、CI 和合并预演见 `research/pr-triage.md`。

## Requirements

| ID | 需求 |
| --- | --- |
| R1 | 用 merge commit 按 #36、#37、#39 的顺序把三个 PR 合并进 `main`。#36 将 `thiserror` 和 `thiserror-impl` 锁到 2.0.21，并带上该 PR 自己的 `syn` 3.0.9。#37 将 `Cargo.toml` 的 `opener` 要求改成 `0.9.0`，锁到 `opener` 0.9.0。#39 将 `clap_complete` 锁到 4.6.11。三者都保留各自 PR 里的 `windows-sys` 锁调整。 |
| R2 | 关闭 #35 和 #38，不合并，每条都留下原因评论。#38 的 `rusqlite` 0.41.1 要求 rustc 1.89.0，`libsqlite3-sys` 0.39.1 要求 rustc 1.88.0。#35 使 `src/zot-local/src/pdf.rs:804` 的 `Ok(Pdfium)` 编不过。 |
| R3 | 处置完成后删除 5 个远端分支：`dependabot/cargo/clap_complete-4.6.11`、`dependabot/cargo/opener-0.9.0`、`dependabot/cargo/pdfium-render-0.9.5`、`dependabot/cargo/rusqlite-0.41.1`、`dependabot/cargo/thiserror-2.0.21`。再删除本克隆的 `refs/remotes/origin/dependabot/*`。`main` 和 `dev` 保留。 |
| R4 | 在 `main` 的 `.github/dependabot.yml` 增加 `rusqlite` `>= 0.40.0` 和 `pdfium-render` `>= 0.9.5`。uuid、sha2、quick-xml、三条 `monthly`、`dtolnay/rust-toolchain` ignore、limit 和 labels 保持不动。这个文件改动提交并推送到 `origin/main`。 |
| R5 | `dev` 的 `Cargo.lock`、`Cargo.toml` 和 `.github/dependabot.yml` 保持现状。不推送 `origin/dev`，也不推送本地 `dev` 上已有的超前提交。 |
| R6 | 保持 `rust-version = "1.85"`。不改 `src/zot-local/src/pdf.rs:804`。不改 audit、deny，也不把安全检查改成通过。 |

## Acceptance Criteria

- AC1 / R1：#36、#37、#39 已合并。`origin/main` 的锁含 `thiserror` 2.0.21、`thiserror-impl` 2.0.21、`syn` 3.0.9、`opener` 0.9.0、`clap_complete` 4.6.11。`Cargo.toml` 的 `opener` 要求是 `0.9.0`。`rusqlite` 仍是 0.39.0，`pdfium-render` 仍是 0.9.0，`clap_complete` 要求仍是 `4.6.1`，`thiserror` 要求仍是 `2.0.18`。
- AC2 / R2：#35、#38 已关闭且没有 merged。各自有一条说明原因的评论。`origin/main` 的 `Cargo.toml` 里 `rusqlite` 要求仍是 `0.39.0`。
- AC3 / R3：`git ls-remote --heads origin dependabot/*` 没有结果。本克隆 `git branch -a` 没有 `dependabot/*`。`main` 和 `dev` 仍在。
- AC4 / R4：`origin/main` 的 cargo ignore 含 `rusqlite` `>= 0.40.0` 和 `pdfium-render` `>= 0.9.5`。uuid `>= 1.24.0`、sha2 `>= 0.11.0`、quick-xml `>= 0.42.0` 仍在。cargo、npm、actions 仍是 `monthly`。actions 仍忽略 `dtolnay/rust-toolchain`。
- AC5 / R5：`origin/dev` 仍是 `963585885ec8eaa52ed7bdf54b4908a97d070ad3`。本地 `HEAD` 仍是 `da2fbfade2f34fca832dfa9a1c3265d435cc1b2e`。工作区里的 `Cargo.lock`、`Cargo.toml`、`.github/dependabot.yml` 与这个 `HEAD` 一致。
- AC6 / R6：本任务自己的提交只改 `.github/dependabot.yml`。#36 和 #39 的合并只改 `Cargo.lock`。#37 的合并只改 `Cargo.toml` 和 `Cargo.lock`。`pdf.rs:804` 没有改动。`rust-version` 仍是 `1.85`。diff 不含 audit ignore 或 `continue-on-error`。

## Out of Scope

- 改 `src/zot-local/src/pdf.rs:804`，或把 Pdfium 下载目标从 7543 调到 8125。
- 把 MSRV 升到 1.88 或 1.89 来接收 `rusqlite` 0.41.1。
- 修复 `main` 上已有的安全检查失败。
- 删除或推送 `dev`，发布，或打 tag。
- 处理推送配置期间 Dependabot 新开的、不属于这 5 个 PR 的更新。
