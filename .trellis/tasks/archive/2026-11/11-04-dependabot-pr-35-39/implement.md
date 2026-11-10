# 实施：合并 #36、#37、#39，关闭 #35 和 #38，清理分支

工作区保持在 `dev`。只在临时 worktree 里提交 `main`。

## 清单

1. `git fetch origin`。确认 5 个 PR 头仍是 `design.md` 里的 SHA。有变化时停在该 PR，重读 diff 和非安全类 CI。
2. `git worktree add --detach <临时目录> origin/main`。只编辑其中的 `.github/dependabot.yml`，把 cargo ignore 改成 `design.md` 里的五块。其余 schedule、limit、labels 和 actions ignore 不动。
3. 在 worktree 里只暂存这个文件，用本地 `git commit -F` 提交。提交说明写明为什么忽略 `rusqlite` `>= 0.40.0`，以及为什么忽略 `pdfium-render` `>= 0.9.5`。不使用 `--trailer`。然后 `git push origin HEAD:main`。
4. `gh pr merge 36 --repo bahayonghang/zotero-cli --merge --match-head-commit 68dc9d5f3e211bbcd457c95f3f00b6ad2363997e`，然后同样方式合并 #37，再合并 #39。不传 `--auto`、`--delete-branch` 或 `--admin`。GitHub 报冲突时停止，不手工改 `Cargo.lock`。
5. 关闭 #38，评论：`关闭，不合并。rusqlite 0.41.1 要求 rustc 1.89.0，libsqlite3-sys 0.39.1 要求 rustc 1.88.0。仓库 MSRV 是 Rust 1.85。分支会删除。`
6. 关闭 #35，评论：`关闭，不合并。pdfium-render 0.9.5 把 Pdfium 改成带字段的结构体。src/zot-local/src/pdf.rs:804 的 Ok(Pdfium) 编不过。仓库 MSRV 保持 Rust 1.85。分支会删除。`
7. 删除 5 个远端 `dependabot/cargo/*` 分支。回到本克隆执行 `git fetch origin --prune`。跟踪引用仍在时，用 `git branch -dr` 删除这 5 个 `origin/dependabot/cargo/*`。
8. 删除临时 worktree。
9. 跑下面的验收命令，把结果记到 `research/disposition.md`。

## 验收命令

```text
git fetch origin main dev
git rev-parse origin/main origin/dev HEAD
git show origin/main:.github/dependabot.yml
git status --short -- Cargo.lock Cargo.toml .github/dependabot.yml
gh pr view 36 --repo bahayonghang/zotero-cli --json state,mergedAt,mergeCommit
gh pr view 37 --repo bahayonghang/zotero-cli --json state,mergedAt,mergeCommit
gh pr view 39 --repo bahayonghang/zotero-cli --json state,mergedAt,mergeCommit
gh pr view 35 --repo bahayonghang/zotero-cli --json state,mergedAt,comments
gh pr view 38 --repo bahayonghang/zotero-cli --json state,mergedAt,comments
git ls-remote --heads origin dependabot/*
git branch -a
```

再用 `git show origin/main:Cargo.lock` 和 `git show origin/main:Cargo.toml` 核对 `prd.md` AC1 的版本。通过标准是 AC1–AC6。不把 `cargo audit` 或安全 job 当作本任务的通过条件。

## 风险点

- 推送目标只能是 `main`。`origin/dev` 停在 `963585885ec8eaa52ed7bdf54b4908a97d070ad3`。本地 `HEAD` 停在 `da2fbfade2f34fca832dfa9a1c3265d435cc1b2e`。
- 三个 PR 的合并预演是干净的。GitHub 仍报冲突时，不手工改锁文件。
- 推送 ignore 后新出现的其他 Dependabot PR 只记录在 `research/disposition.md`。
- 不修改 `ci.yml`、`pdf.rs`、rusqlite 要求、pdfium-render 要求和 audit 配置。

## 回滚点

先 revert `dependabot.yml` 提交并推送 `main`。若补丁本身有问题，再按 #39、#37、#36 的逆序 revert merge commit。分支头见 `design.md`。
