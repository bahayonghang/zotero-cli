# 处置剩余 Dependabot PR 并清理分支

## Goal

把仍开着的 5 个 Dependabot PR 处置完，让 `main` 得到能通过 Rust 1.85 的锁更新，并删掉它们留下的远端分支和本克隆里的跟踪引用。

## Background

仓库是 `bahayonghang/zotero-cli`。默认分支是 `main`。2026-11-04 复核时 `origin/main` 为 `e04a7f8817713eb7ae132d082ba3f401d1390d4a`，`origin/dev` 为 `963585885ec8eaa52ed7bdf54b4908a97d070ad3`。本地 `dev` 停在 `e3926bf`，比 `origin/dev` 超前 3 个任务归档提交。

同日已归档的 `11-04-dependabot-monthly` 把 #30–#34 留在范围外。`main` 的 `.github/dependabot.yml` 已是 monthly，并忽略 uuid `>= 1.27.0` 与 quick-xml `>= 0.42.0`。本地 `dev` 上的同名文件仍是 cargo / npm weekly，没有这两条 ignore。

这 5 个 PR 的 base 都是 `main`，作者都是 Dependabot，`mergeable` 为 `MERGEABLE`，`mergeStateStatus` 为 `UNSTABLE`。`deleteBranchOnMerge` 为 false。`main` 没有分支保护，ruleset 数量为 0。允许 merge、squash、rebase。历史合并使用 merge commit。安全检查失败来自 `main` 上原有的 quinn-proto、rustls 和 webpki 漏洞。

工作区要求在根 `Cargo.toml`：第 17 行 `rust-version = "1.85"`，第 29 行 md5 `0.8.0`，第 46 行 serde `1.0.228`，第 48 行 sha2 `0.10.9`，第 55 行 toml `1.1.2`，第 57 行 uuid `1.23.1`。`dev` 与 `origin/main` 上这五个包的锁版本目前相同。

本克隆没有 `dependabot/*` 本地分支，只有 5 个 `remotes/origin/dependabot/*` 跟踪引用。版本、CI 和合并预演见 `research/pr-triage.md`。

## Requirements

| ID | 需求 |
| --- | --- |
| R1 | 用 merge commit 按 #31、#32、#34 的顺序把三个 PR 合并进 `main`。#31 将 serde、serde_core、serde_derive 锁到 1.0.229。#32 将 md5 锁到 0.8.1。#34 将 toml 锁到 `1.2.0+spec-1.1.0`，并带上该 PR 自己的 serde_spanned 1.1.2、toml_datetime `1.1.2+spec-1.1.0`、toml_parser `1.1.5+spec-1.1.0`、toml_writer `1.2.0+spec-1.1.0`。三者都只改 `Cargo.lock`。 |
| R2 | 关闭 #30 和 #33，不合并，每条都留下原因评论。#30 的锁是 uuid 1.29.0，MSRV 作业报 `uuid@1.29.0 requires rustc 1.89.0`。#33 把 sha2 要求改成 0.11.1，digest 升到 0.11.3，`src/zot-local/src/pdf.rs:788`、`src/zot-cli/src/commands/organize.rs:1157`、`src/zot-cli/tests/json_error_contract.rs:208` 的 `format!("{:x}")` 编不过。 |
| R3 | 处置完成后删除 5 个远端分支：`dependabot/cargo/uuid-1.26.1`、`dependabot/cargo/serde-1.0.229`、`dependabot/cargo/md5-0.8.1`、`dependabot/cargo/sha2-0.11.1`、`dependabot/cargo/toml-1.1.8spec-1.1.0`。再删除本克隆的 `refs/remotes/origin/dependabot/*`。`main` 和 `dev` 保留。 |
| R4 | 在 `main` 的 `.github/dependabot.yml` 把 uuid ignore 从 `>= 1.27.0` 改为 `>= 1.24.0`，并增加 sha2 `>= 0.11.0`。quick-xml `>= 0.42.0`、三条 `monthly`、`dtolnay/rust-toolchain` ignore、limit 和 labels 保持不动。这个文件改动提交并推送到 `origin/main`。 |
| R5 | `dev` 的 `Cargo.lock`、`Cargo.toml` 和 `.github/dependabot.yml` 保持现状。不推送 `origin/dev`，也不推送本地 `dev` 上已有的 3 个归档提交。 |
| R6 | 保持 `rust-version = "1.85"`。不改 audit、deny，也不把安全检查改成通过。不修改 R2 里的三处哈希格式化调用。 |

## Acceptance Criteria

- AC1 / R1：#31、#32、#34 已合并。`origin/main` 的 `Cargo.lock` 含 serde 1.0.229、serde_core 1.0.229、serde_derive 1.0.229、md5 0.8.1、toml `1.2.0+spec-1.1.0`，以及 R1 列出的 serde_spanned 和 toml_* 版本。sha2 仍是 0.10.9，uuid 仍是 1.23.1。
- AC2 / R2：#30、#33 已关闭且没有 merged。各自有一条说明原因的评论。`origin/main` 的 `Cargo.toml` 里 sha2 要求仍是 `0.10.9`。
- AC3 / R3：`git ls-remote --heads origin dependabot/*` 没有结果。本克隆 `git branch -a` 没有 `dependabot/*`。`main` 和 `dev` 仍在。
- AC4 / R4：`origin/main` 的 `dependabot.yml` 中，cargo ignore 含 sha2 `>= 0.11.0` 和 uuid `>= 1.24.0`，quick-xml `>= 0.42.0` 仍在。cargo、npm、actions 仍是 `monthly`。actions 仍忽略 `dtolnay/rust-toolchain`。
- AC5 / R5：`origin/dev` 仍是 `963585885ec8eaa52ed7bdf54b4908a97d070ad3`。本地 `HEAD` 仍是 `e3926bf`。工作区里的 `Cargo.lock`、`Cargo.toml`、`.github/dependabot.yml` 与这个 `HEAD` 一致。
- AC6 / R6：本任务自己的提交只改 `.github/dependabot.yml`。三次合并只改 `Cargo.lock`。`pdf.rs:788`、`organize.rs:1157`、`json_error_contract.rs:208` 没有改动。`rust-version` 仍是 `1.85`。diff 不含 audit ignore 或 `continue-on-error`。

## Out of Scope

- 把 sha2 0.11 的 `format!("{:x}")` 改成新 API。
- 修复 `main` 上已有的 rustls、quinn-proto、webpki 漏洞。
- 推送 `origin/dev`、发布或打 tag。
- 把 `dev` 上仍为 weekly 的 Dependabot 配置改成 `main` 的 monthly。
- 处理推送配置期间 Dependabot 新开的、不属于这 5 个 PR 的更新。
