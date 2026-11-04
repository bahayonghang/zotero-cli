# 设计：合并三个锁更新，关闭两个不兼容 PR，并清理分支

## 边界

代码落地分支是 `main`。本地工作区停留在 `dev`。`.github/dependabot.yml` 只在从 `origin/main` 拉出的临时 detached worktree 里修改并推送，避免把 `dev` 上的任务文件或 weekly 配置带上 `main`。

| 改动 | 落点 | 保持不动 |
| --- | --- | --- |
| #31 serde 1.0.229 | GitHub merge commit 进入 `main`，只含该 PR 的 `Cargo.lock` | `Cargo.toml`、`dev` |
| #32 md5 0.8.1 | 同上 | 同上 |
| #34 toml 1.2.0 | 同上。锁里同时包含该 PR 自己的 toml_* 与 serde_spanned | `dev` |
| #30、#33 | 评论后关闭，删除分支 | 锁文件、`Cargo.toml`、三处 `format!("{:x}")` |
| Dependabot ignore | `main` 的 `.github/dependabot.yml` | `dev` 的同名文件、`ci.yml`、audit 配置 |

2026-11-04 的 `git merge-tree --write-tree` 按 #31、#32、#34 顺序合并，三次都干净。合成锁只等于三份 diff 的并集，sha2 与 uuid 不变。`main` 无保护，失败的安全检查不会挡住 merge。

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
```

uuid 的忽略范围从 `>= 1.27.0` 扩大到 `>= 1.24.0`，因为 #30 的标题是 1.26.1，锁却解到 1.29.0，旧规则挡不住。sha2 `>= 0.11.0` 挡住会破坏 `LowerHex` 的大版本。quick-xml、三条 monthly、actions 的 `dtolnay/rust-toolchain` ignore、limit 和 labels 不动。

## 合并与关闭

使用 `gh pr merge <n> --merge`。不使用 `--auto`、`--squash`、`--rebase` 或 `--delete-branch`。顺序是 #31、#32、#34。

只合并设计里记下的头。头变了就先重读 diff 和非安全类 CI，再决定能不能合。

| PR | 分支 | 头 |
| --- | --- | --- |
| #30 | `dependabot/cargo/uuid-1.26.1` | `fd15c9d11f285e7ea9d9a0c521ed8e5b88192bc3` |
| #31 | `dependabot/cargo/serde-1.0.229` | `e8f9439d614887abfa06f8ea4afe822014ffe435` |
| #32 | `dependabot/cargo/md5-0.8.1` | `de6c6c20ae37fdb30ce0a0b156b5cc79bc42464a` |
| #33 | `dependabot/cargo/sha2-0.11.1` | `c4d66c0a5e530dd68362a724b5074873a61271aa` |
| #34 | `dependabot/cargo/toml-1.1.8spec-1.1.0` | `720410c1fd865542fa86ab5f7d3415ce50e9cd71` |

#30 的评论说明锁里是 uuid 1.29.0，要求 rustc 1.89.0。#33 的评论说明 `pdf.rs:788`、`organize.rs:1157`、`json_error_contract.rs:208` 的 `format!("{:x}")` 编不过。Dependabot 若已先关闭 PR，仍然补上这条评论。

`deleteBranchOnMerge` 为 false。评论和关闭之后再删除 5 个远端分支，然后在本克隆 `git fetch origin --prune`。若跟踪引用还在，再 `git branch -dr` 这 5 个 `origin/dependabot/cargo/*`。不删除 `main` 或 `dev`。

## 回滚

`dependabot.yml` 提交用普通 revert，再推送 `main`。已合并的 PR 按 #34、#32、#31 的逆序 revert merge commit。已关闭的 PR 可以重开。已删除的分支用上表的头重建。不做 force push。

## 风险

合并后 `main` 的安全检查仍会失败。这是既有漏洞，本任务不改 audit 配置。

推送 `dependabot.yml` 可能让 Dependabot 再开新的 PR。不属于这 5 个编号的新 PR 只记录，不合并、不关闭。

`dev` 的 `dependabot.yml` 仍是 weekly。以后把 `dev` 合并进 `main` 时，可能把 monthly 和这三条 ignore 盖回去。本次按选定方案不推送 `dev`。

`dev` 不会得到 serde 1.0.229、md5 0.8.1 和 toml 1.2.0。
