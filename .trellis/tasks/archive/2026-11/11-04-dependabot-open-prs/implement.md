# 实施：合并 #31、#32、#34，关闭 #30 和 #33，清理分支

工作区保持在 `dev`。只在临时 worktree 里提交 `main`。

## 清单

1. `git fetch origin`。确认 5 个 PR 头仍是 `design.md` 里的 SHA。有变化时停在该 PR，重读 diff 和非安全类 CI。
2. `git worktree add --detach <临时目录> origin/main`。只编辑其中的 `.github/dependabot.yml`，把 cargo ignore 改成 `design.md` 里的三块。其余 schedule、limit、labels 和 actions ignore 不动。
3. 在 worktree 里只暂存这个文件，用本地 `git commit -F` 提交。提交说明写明为什么把 uuid 忽略扩到 `>= 1.24.0`，以及为什么忽略 sha2 `>= 0.11.0`。不使用 `--trailer`。然后 `git push origin HEAD:main`。
4. `gh pr merge 31 --merge`，然后 #32，然后 #34。不传 `--auto` 或 `--delete-branch`。GitHub 报冲突时停止，不手工改 `Cargo.lock`。
5. 关闭 #30，评论：`关闭，不合并。标题写的是 uuid 1.26.1，Cargo.lock 实际把 uuid 解到 1.29.0。1.29.0 要求 rustc 1.89.0，仓库 MSRV 是 Rust 1.85。分支会删除。`
6. 关闭 #33，评论：`关闭，不合并。sha2 0.11.1 把 digest 升到 0.11.3。src/zot-local/src/pdf.rs:788、src/zot-cli/src/commands/organize.rs:1157、src/zot-cli/tests/json_error_contract.rs:208 的 format!("{:x}") 编不过。仓库 MSRV 保持 Rust 1.85。分支会删除。`
7. 删除 5 个远端 `dependabot/cargo/*` 分支。回到本克隆执行 `git fetch origin --prune`。跟踪引用仍在时，用 `git branch -dr` 删除这 5 个 `origin/dependabot/cargo/*`。
8. 删除临时 worktree。
9. 跑下面的验收命令，把结果记到 `research/disposition.md`。

## 验收命令

```text
git fetch origin main dev
git rev-parse origin/main origin/dev HEAD
git show origin/main:.github/dependabot.yml
git status --short -- Cargo.lock Cargo.toml .github/dependabot.yml
gh pr view 31 --json state,mergedAt
gh pr view 32 --json state,mergedAt
gh pr view 34 --json state,mergedAt
gh pr view 30 --json state,mergedAt,comments
gh pr view 33 --json state,mergedAt,comments
git ls-remote --heads origin dependabot/*
git branch -a
```

再用 `git show origin/main:Cargo.lock` 核对 `prd.md` AC1 的版本。通过标准是 AC1–AC6。不把 `cargo audit` 或安全 job 当作本任务的通过条件。

## 风险点

- 推送目标只能是 `main`。`origin/dev` 停在 `963585885ec8eaa52ed7bdf54b4908a97d070ad3`。
- 三个 PR 的合并预演是干净的。GitHub 仍报冲突时，不手工改锁文件。
- 推送 ignore 后新出现的其他 Dependabot PR 只记录在 `research/disposition.md`。
- 不修改 `ci.yml`、`Cargo.toml`、三处哈希格式化调用和 audit 配置。

## 回滚点

先 revert `dependabot.yml` 提交并推送 `main`。若补丁本身有问题，再按 #34、#32、#31 的逆序 revert merge commit。分支头见 `design.md`。
