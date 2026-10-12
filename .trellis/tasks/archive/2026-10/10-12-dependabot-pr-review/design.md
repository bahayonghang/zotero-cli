# 设计：Dependabot PR 处置

## 边界

工作分支是本地 `dev`。八个 GitHub PR 保持以 `main` 为 base，不在 GitHub 上按 merge 按钮，也不把它们 rebase 到 `dev`。等价改动由本地提交产生，评论里写明本地提交哈希，并说明这些提交还没有推送。

| 子任务 | 可改文件 | 不改 |
| --- | --- | --- |
| crate bumps | `Cargo.lock` | `Cargo.toml`、workflow |
| dirs | `Cargo.toml` 的 `dirs` 要求、`Cargo.lock` | 调用点、`AGENTS.md` |
| actions | `.github/workflows/ci.yml`、`.github/workflows/deploy-docs.yml` 里的 checkout 与 setup-node 引用 | toolchain 引用、`node-version`、其他 action |
| MSRV | `.github/dependabot.yml` | `ci.yml` 的 `@1.85.0`、`Cargo.toml` 的 `rust-version` |

父任务不改产品文件。它在四个子任务之后核对锁版本、workflow 引用、`just ci` 和 PR 状态，并把汇总写到 `research/disposition.md`。

## 锁文件

只使用带包名和当前版本的精确更新：

```text
cargo update -p clap@4.6.1 --precise 4.6.6
cargo update -p thiserror@2.0.18 --precise 2.0.20
cargo update -p tar@0.4.45 --precise 0.4.46
cargo update -p http@1.4.0 --precise 1.5.0
```

dirs 先把 workspace 要求从 `6.0.0` 改为 `7.0.0`，再执行 `cargo update -p dirs@6.0.0 --precise 7.0.0`。

`cargo update` 若为了满足目标版本而改动该 crate 的传递依赖，可以保留，但必须在子任务记录里列出。`rustls` 版本变化、thiserror 1.0.69 消失，或其他直接依赖版本变化，都要停下来，不扩大升级范围。

## 兼容性

- checkout v7 与 setup-node v7 的现有输入可以原样使用。依据是发布说明和 2026-10-01 已通过的功能 job，详见 `research/pr-triage.md`。
- dirs 7 相对 6 只改变 Windows `preference_dir`。本仓库不调用它。
- http 1.5.0 是 reqwest / hyper 的传递依赖。PathAndQuery 上限是 `u16::MAX - 1` 字节。不新增直接依赖。
- tar 0.4.46 修 GHSA-3cv2-h65g-fgmm，调用点仍是 `src/zot-local/src/pdf.rs`。

## Dependabot

在 `github-actions` 条目下忽略整个 `dtolnay/rust-toolchain`。`@1.85.0`、`@stable`、`@nightly` 都是工具链选择器，不是 action 的 semver。忽略范围不能波及 `actions/checkout` 或 `actions/setup-node`。

## 回滚

每个子任务单独提交。回滚该提交即可恢复它的文件。已关闭的 PR 不重新打开，除非回滚后用户要求恢复；评论要保留。

## 风险

本地 `dev` 比 `origin/dev` 超前。关闭 GitHub PR 后，远程在推送前看不到这些升级。这是本轮不推送的结果，评论里要写明。
