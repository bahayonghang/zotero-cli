# 处置结果

日期：2026-11-04。落地分支：`origin/main`，头为 `f49662e7c4b0f302b83ecef7b1fb3c510ef9b7b2`。`origin/dev` 仍是 `963585885ec8eaa52ed7bdf54b4908a97d070ad3`。本地 `HEAD` 仍是 `e3926bf`。本地 `Cargo.lock`、`Cargo.toml`、`.github/dependabot.yml` 相对这个 `HEAD` 没有改动。

| 项 | 结果 |
| --- | --- |
| 配置提交 | `0ddad44dd00758af561a2adaf799f472ec677094` `ci(dependabot): 👷 忽略越过 Rust 1.85 的 sha2 与 uuid`，已推送 `origin/main` |
| #31 serde 1.0.229 | 已合并。merge commit `2167fa00d4dce844e80c354bb9faf8c31745e084`。头 `e8f9439d614887abfa06f8ea4afe822014ffe435` |
| #32 md5 0.8.1 | 已合并。merge commit `0b2f679d94c4b8a5a35d500b0779d6db02cda681`。头 `de6c6c20ae37fdb30ce0a0b156b5cc79bc42464a` |
| #34 toml 1.2.0 | 已合并。merge commit `f49662e7c4b0f302b83ecef7b1fb3c510ef9b7b2`。头 `720410c1fd865542fa86ab5f7d3415ce50e9cd71`。`gh pr merge` 返回 502，随后读取到的状态是已合并 |
| #30 uuid | 未合并，已关闭。Dependabot 在忽略生效后先关闭，随后补了原因评论：https://github.com/bahayonghang/zotero-cli/pull/30#issuecomment-6628193966 |
| #33 sha2 | 未合并，已关闭。同样先被 Dependabot 关闭，随后补了评论：https://github.com/bahayonghang/zotero-cli/pull/33#issuecomment-6628194018 |

`origin/main` 的 `Cargo.lock`：serde 1.0.229、serde_core 1.0.229、serde_derive 1.0.229、md5 0.8.1、serde_spanned 1.1.2、toml `1.2.0+spec-1.1.0`、toml_datetime `1.1.2+spec-1.1.0`、toml_parser `1.1.5+spec-1.1.0`、toml_writer `1.2.0+spec-1.1.0`、sha2 0.10.9、uuid 1.23.1、digest 0.10.7。

`origin/main` 的 `Cargo.toml` 要求仍是 `rust-version = "1.85"`、sha2 `0.10.9`。`e04a7f8..origin/main` 只改了 `.github/dependabot.yml` 和 `Cargo.lock`。

`origin/main` 的 `dependabot.yml`：cargo、npm、actions 仍是 `monthly`。cargo ignore 为 uuid `>= 1.24.0`、sha2 `>= 0.11.0`、quick-xml `>= 0.42.0`。actions 仍忽略 `dtolnay/rust-toolchain`。

原先 5 个分支都已不在远程，本克隆也没有它们的跟踪引用。`dependabot/cargo/md5-0.8.1` 是合并后手工删除的。serde、toml、uuid、sha2 在关闭或合并后已不存在。

推送配置后 Dependabot 立刻再跑了一次，并打开了本任务范围外的 5 个 PR。它们没有被合并、关闭或删除：

| PR | 分支 | 标题 |
| --- | --- | --- |
| #35 | `dependabot/cargo/pdfium-render-0.9.5` | bump pdfium-render from 0.9.0 to 0.9.5 |
| #36 | `dependabot/cargo/thiserror-2.0.21` | bump thiserror from 2.0.18 to 2.0.21 |
| #37 | `dependabot/cargo/opener-0.9.0` | bump opener from 0.8.4 to 0.9.0 |
| #38 | `dependabot/cargo/rusqlite-0.41.1` | bump rusqlite from 0.39.0 to 0.41.1 |
| #39 | `dependabot/cargo/clap_complete-4.6.11` | bump clap_complete from 4.6.2 to 4.6.11 |

因此 `git ls-remote --heads origin dependabot/*` 现在还有这 5 个新分支。临时 worktree 已删除。
