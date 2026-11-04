# 设计：合并三个 Dependabot PR，并改为每月检查

## 边界

代码落地分支是 `main`。本地工作区停留在 `dev`，用临时 worktree 修改并推送 `main`，避免带走 `dev` 上未提交的任务文件。

| 改动 | 落点 | 不改 |
| --- | --- | --- |
| #25 chrono 0.4.45 | GitHub merge commit 进入 `main`，只含该 PR 的 `Cargo.lock` | `Cargo.toml`、`dev` |
| #26 flate2 1.1.10 | 同上 | 同上 |
| #27 clap 4.6.7 | 同上。`main` 上的 clap 从 4.6.1 到 4.6.7 | `dev` 上已经的 4.6.6 |
| #28、#29 | 评论后关闭，删除分支 | 锁文件和 `Cargo.toml` |
| Dependabot 日程和 ignore | `main` 的 `.github/dependabot.yml` | `dev` 的同名文件、`ci.yml` |

2026-11-04 用 `git merge-tree --write-tree` 按 #25、#26、#27 的顺序合并，三次都干净。`main` 无保护，失败的安全检查不会挡住 merge。检查状态是 `UNSTABLE`，原因是 `main` 上原有的 rustls 0.23.38 和 quinn-proto 0.11.15，不是这三个补丁引入的。

## 合并

使用 `gh pr merge <n> --merge`。不使用 `--auto`、`--squash` 或 `--rebase`。顺序是 #25、#26、#27。先推送 `dependabot.yml`，再合并；这个文件和三个 PR 的 `Cargo.lock` 不相交。

`deleteBranchOnMerge` 为 false。合并或关闭之后单独删除 5 个远程分支。分支头保留在研究记录里，便于从 PR head 恢复：

| 分支 | 头 |
| --- | --- |
| `dependabot/cargo/chrono-0.4.45` | `fd17fe194d17ffb6dabf2a88eff49f4728956ddc` |
| `dependabot/cargo/flate2-1.1.10` | `9ba3047f008d3818964f7c331518d0dbfae15cae` |
| `dependabot/cargo/clap-4.6.7` | `8ce3efc5ceec82bf6262244baaf0a2b40e052401` |
| `dependabot/cargo/uuid-1.28.0` | `c320dfc97810b60326643a59ac24d88ca7218e6c` |
| `dependabot/cargo/quick-xml-0.42.0` | `1cd8c866498ff1e71b4f4de263e7f6813830ef4e` |

## Dependabot

`main` 当前的 cargo 与 npm 是 `weekly`，actions 已是 `monthly`，但没有 `dtolnay/rust-toolchain` ignore。`dev` 有这条 ignore，Dependabot 读的是默认分支 `main`。

`monthly` 在每月 1 日运行。不设置 `time` 或 `timezone`。limit 和 labels 保持原样。

cargo 的 ignore 使用 Cargo version requirement：

- uuid `>= 1.27.0`：1.27.0 和 1.28.0 的 crates.io rust-version 都是 1.89.0。1.26.0 仍是 1.85.0，这次不升级，也不忽略。
- quick-xml `>= 0.42.0`：0.42.0 要求 rustc 1.86，并破坏 `oa.rs` 的 0.41 API。

actions 的 ignore 沿用 `dev` 上已有的整名忽略和同一句注释。

## 回滚

`dependabot.yml` 提交用普通 revert，再推送 `main`。已合并的 PR 各自 revert 其 merge commit。已关闭的 PR 可以重开。已删除的分支可以用上表的头重建。不做 force push。

## 风险

合并后 `main` 的安全检查仍会失败。这是既有漏洞，本任务不改 audit 配置，也不把失败标成通过。

`dev` 的 `dependabot.yml` 仍是 weekly。以后把 `dev` 合并进 `main` 时，这个文件可能冲突，或把每月检查盖回去。本次按选定方案不推送 `dev`。

`dev` 与 `main` 的锁文件会继续分叉。`dev` 不会得到 chrono 0.4.45、flate2 1.1.10 和 clap 4.6.7。
