# 实施：合并 #25–#27，关闭 #28–#29，改为每月检查

工作区保持在 `dev`。只在临时 worktree 里提交 `main`。

## 清单

1. 从 `origin/main` 建 detached worktree。编辑其中的 `.github/dependabot.yml`，使 cargo 与 npm 的 `interval` 为 `monthly`，actions 保持 `monthly`，并加入 design.md 里的三条 ignore。limit 和 labels 不动。
2. 在 worktree 提交该文件并 `git push origin HEAD:main`。提交说明写明为什么把检查改成每月，以及为什么忽略这两条越界版本。不使用 `--trailer`。
3. `gh pr merge 25 --merge`，然后 #26，然后 #27。不传 `--auto`。
4. 关闭 #28，评论：`关闭，不合并。uuid 1.28.0 要求 rustc 1.89.0，仓库 MSRV 是 Rust 1.85。1.27.0 起同样要求 1.89。分支会删除。`
5. 关闭 #29，评论：`关闭，不合并。quick-xml 0.42.0 要求 rustc 1.86，并且 src/zot-remote/src/oa.rs 在 0.42 上编译失败。仓库 MSRV 保持 Rust 1.85。分支会删除。`
6. 删除 5 个 `dependabot/cargo/*` 远程分支。
7. 删除临时 worktree。
8. 跑下面的验收命令，把结果记到 `research/disposition.md`。

## 验收命令

```text
git fetch origin main dev
git show origin/main:.github/dependabot.yml
git show origin/main:Cargo.lock
git rev-parse origin/dev
git status --short -- Cargo.lock Cargo.toml
gh pr view 25 --json state,mergedAt
gh pr view 26 --json state,mergedAt
gh pr view 27 --json state,mergedAt
gh pr view 28 --json state,mergedAt,comments
gh pr view 29 --json state,mergedAt,comments
git ls-remote --heads origin dependabot/*
```

通过标准见 `prd.md` 的 AC1–AC6。不把 `cargo audit` 或 `just ci` 的安全 job 当作本任务的通过条件。

## 风险点

- 推送目标只能是 `main`。`origin/dev` 停在 `9635858`。
- 三个 PR 的合并如果被 GitHub 报冲突，先对照 `git merge-tree` 的干净结果，不手工改锁文件。
- 不修改 `ci.yml`、`Cargo.toml` 和 audit 配置。

## 回滚点

先 revert `dependabot.yml` 提交。若补丁本身有问题，再按合并的逆序 revert #27、#26、#25。分支头见 `design.md`。
