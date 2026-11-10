# 处置结果

执行时 5 个 PR 的头与 `design.md` 一致。本地工作区停留在 `dev`。

## 已落到 main

| 动作 | 结果 | 提交 |
| --- | --- | --- |
| ignore 推送到 `main` | `rusqlite >= 0.40.0`，`pdfium-render >= 0.9.5` | `17598e9b55d3cd1b5cb611fe9b6592c95cf70adb` |
| #36 thiserror 2.0.21 | MERGED | `1f454164501e0a1e028861f5306d47dbb0cc6d4a` |
| #37 opener 0.9.0 | MERGED | `cf38c162df389774ce10cf24e4fe9dd208e6a5f4` |
| #39 clap_complete 4.6.11 | MERGED | `6e9f1249044ccafae94840231251d74426ca9c97` |
| #38 rusqlite 0.41.1 | CLOSED，`mergedAt` 为空 | 已写关闭原因 |
| #35 pdfium-render 0.9.5 | CLOSED，`mergedAt` 为空 | 已写关闭原因 |

Dependabot 在 ignore 推送后先关闭了 #35 和 #38，评论是 “no longer being updated by Dependabot”。随后补上了计划中的两条中文原因。

`origin/main` 现在是 `6e9f1249044ccafae94840231251d74426ca9c97`。相对任务开始时的 `f49662e7c4b0f302b83ecef7b1fb3c510ef9b7b2`，diff 只有 `.github/dependabot.yml`、`Cargo.lock`、`Cargo.toml`。`Cargo.toml` 只把 `opener` 从 `0.8.4` 改成 `0.9.0`。

`origin/main` 的锁：`thiserror` 2.0.21，`thiserror-impl` 2.0.21，`syn` 3.0.9 与原有 `syn` 2.0.117，`opener` 0.9.0，`clap_complete` 4.6.11，`pdfium-render` 0.9.0，`rusqlite` 0.39.0。`Cargo.toml` 里 `clap_complete` 仍是 `4.6.1`，`thiserror` 仍是 `2.0.18`，`rusqlite` 仍是 `0.39.0`，`pdfium-render` 仍是 `0.9.0`，`rust-version` 仍是 `1.85`。`src/zot-local/src/pdf.rs` 仍有 `Ok(Pdfium)`。

## 原分支

这 5 个远端分支已不存在：

- `dependabot/cargo/clap_complete-4.6.11`
- `dependabot/cargo/opener-0.9.0`
- `dependabot/cargo/pdfium-render-0.9.5`
- `dependabot/cargo/rusqlite-0.41.1`
- `dependabot/cargo/thiserror-2.0.21`

本克隆里对应的 `origin/dependabot/cargo/*` 跟踪引用也已去掉。`main` 和 `dev` 还在。

## 保持不动

`origin/dev` 仍是 `963585885ec8eaa52ed7bdf54b4908a97d070ad3`。本地 `HEAD` 仍是 `da2fbfade2f34fca832dfa9a1c3265d435cc1b2e`。工作区的 `Cargo.lock`、`Cargo.toml`、`.github/dependabot.yml` 与这个 `HEAD` 没有差异。临时 worktree 已删除。

## 计划外的新 PR

ignore 推送并清掉旧 PR 后，Dependabot 按每月上限又开了 5 个。这次没有合并，没有关闭，也没有删它们的分支。

| PR | 分支 | 头 |
| --- | --- | --- |
| #40 | `dependabot/cargo/regex-1.13.1` | `aa45845b2649dfa5bc384f2239f77fee0e66f609` |
| #41 | `dependabot/cargo/pdfium-render-0.9.4` | `cbcb4e079b17f2104cdb8a9758a22589762bf7e8` |
| #42 | `dependabot/cargo/serde_json-1.0.154` | `ae01a0fd95fd526123f8ef8d2907e2eb7cedafcb` |
| #43 | `dependabot/cargo/uuid-1.23.5` | `9bad8b161a26e13cc4c42b693f67516b320bf429` |
| #44 | `dependabot/cargo/tokio-1.54.1` | `46a07f00f7e1e3e964fc34619a01cd3f2c7dde17` |

#41 是 0.9.4，低于这次加上的 `pdfium-render >= 0.9.5` 忽略线。#43 是 uuid 1.23.5，低于已有的 `uuid >= 1.24.0` 忽略线。
