# 设计：合并三个兼容更新，关闭两个不兼容 PR，并清理分支

## 边界

代码落地分支是 `main`。本地工作区停留在 `dev`。`.github/dependabot.yml` 只在从 `origin/main` 拉出的临时 detached worktree 里修改并推送，避免把 `dev` 上的任务文件或 weekly 配置带上 `main`。

| 改动 | 落点 | 保持不动 |
| --- | --- | --- |
| #36 thiserror 2.0.21 | GitHub merge commit 进入 `main`，只含该 PR 的 `Cargo.lock` | `Cargo.toml` 的 thiserror 要求、`dev` |
| #37 opener 0.9.0 | merge commit。含该 PR 的 `Cargo.toml` 与 `Cargo.lock` | `dev` |
| #39 clap_complete 4.6.11 | merge commit，只含该 PR 的 `Cargo.lock` | `Cargo.toml` 的 clap_complete 要求、`dev` |
| #35、#38 | 评论后关闭，删除分支 | `pdf.rs:804`、`Cargo.toml` 的 rusqlite 与 pdfium-render 要求 |
| Dependabot ignore | `main` 的 `.github/dependabot.yml` | `dev` 的同名文件、`ci.yml`、audit 配置 |

2026-11-04 的本地预演按 #36、#37、#39 顺序合并，三次都干净。合成锁里 `pdfium-render` 仍是 0.9.0，`rusqlite` 仍是 0.39.0。`main` 无保护，失败的安全检查不会挡住 merge。

## 配置

先推送 ignore，再合并和关闭。这样随后的 Dependabot 运行会看到新规则。cargo 的 ignore 变成：

```yaml
ignore:
  - dependency-name: "uuid"
    versions: [">= 1.24.0"]
  - dependency-name: "sha2"
    versions: [">= 0.11.0"]
  - dependency-name: "quick-xml"
    versions: [">= 0.42.0"]
  - dependency-name: "rusqlite"
    versions: [">= 0.40.0"]
  - dependency-name: "pdfium-render"
    versions: [">= 0.9.5"]
```

`rusqlite` `>= 0.40.0` 挡住 0.40 起高于 Rust 1.85 的 MSRV。`pdfium-render` `>= 0.9.5` 只挡住已经证明编不过的版本。uuid、sha2、quick-xml、三条 monthly、actions 的 `dtolnay/rust-toolchain` ignore、limit 和 labels 不动。

## 合并与关闭

使用 `gh pr merge <n> --repo bahayonghang/zotero-cli --merge --match-head-commit <sha>`。不使用 `--auto`、`--squash`、`--rebase`、`--delete-branch` 或 `--admin`。顺序是 #36、#37、#39。

只合并下表记下的头。头变了就先重读 diff 和非安全类 CI，再决定能不能合。

| PR | 分支 | 头 |
| --- | --- | --- |
| #35 | `dependabot/cargo/pdfium-render-0.9.5` | `4590e9b04d22624afabd9ba02913f4b150634011` |
| #36 | `dependabot/cargo/thiserror-2.0.21` | `68dc9d5f3e211bbcd457c95f3f00b6ad2363997e` |
| #37 | `dependabot/cargo/opener-0.9.0` | `a2ad0540fb21621c688922eeb185849eb39b33d4` |
| #38 | `dependabot/cargo/rusqlite-0.41.1` | `71280ddf3d11b26022e7c3f1724dc6eea0e139b3` |
| #39 | `dependabot/cargo/clap_complete-4.6.11` | `49acbba05f7b8d058a2887ddd6c194904ee39c2d` |

#38 的评论说明 `rusqlite` 0.41.1 要求 rustc 1.89.0，`libsqlite3-sys` 0.39.1 要求 rustc 1.88.0。#35 的评论说明 `src/zot-local/src/pdf.rs:804` 的 `Ok(Pdfium)` 在 0.9.5 编不过。Dependabot 若已先关闭 PR，仍然补上这条评论。

`deleteBranchOnMerge` 为 false。评论和关闭之后再删除 5 个远端分支，然后在本克隆 `git fetch origin --prune`。若跟踪引用还在，再 `git branch -dr` 这 5 个 `origin/dependabot/cargo/*`。不删除 `main` 或 `dev`。

## 回滚

`dependabot.yml` 提交用普通 revert，再推送 `main`。已合并的 PR 按 #39、#37、#36 的逆序 revert merge commit。已关闭的 PR 可以重开。已删除的分支用上表的头重建。不做 force push。

## 风险

合并后 `main` 的安全检查仍会失败。这是既有漏洞，本任务不改 audit 配置。

#37 和 #39 的锁里有 `windows-sys` 重排。预演和这两个 PR 自己的 Windows CI 都通过。GitHub 仍报冲突时，不手工改 `Cargo.lock`。

推送 `dependabot.yml` 可能让 Dependabot 再开新的 PR。不属于这 5 个编号的新 PR 只记录，不合并、不关闭。

`dev` 的 `dependabot.yml` 仍是 weekly。以后把 `dev` 合并进 `main` 时，可能把 monthly 和这些 ignore 盖回去。本次不推送 `dev`。

`dev` 不会得到 thiserror 2.0.21、opener 0.9.0 和 clap_complete 4.6.11。
