# 设计：Actions v7

## 边界

文本替换限于：

- `.github/workflows/ci.yml`：六处 `actions/checkout@v4`，一处 `actions/setup-node@v4`
- `.github/workflows/deploy-docs.yml`：一处 checkout，一处 setup-node

保留 `fetch-depth: 0`、`node-version: 20`、`cache: npm` 和 `cache-dependency-path`。不增加 `package-manager-cache`。`docs/package.json` 没有 `packageManager`，setup-node v5 的自动缓存不会开启；显式的 npm cache 就是当前行为。

## 已核对的大版本行为

- checkout v5 要求 GitHub runner ≥ 2.327.1。#22 在 2026-10-01 的 ubuntu、windows、macos stable job 已用 v7 通过。
- checkout v6 把凭据写入单独文件。同一 PR 的 stable job 执行了 `git diff --exit-code` 并且成功，所以凭据文件没有弄脏工作区。
- checkout v7 拒绝在 `pull_request_target` 和 `workflow_run` 中检出 fork PR。这两个事件不在本仓库 workflow 里。
- setup-node v6 把自动缓存限制为 npm。v7 不改变 `node-version` 或 `cache` 输入。docs job 已通过。

## 回滚

revert 该 workflow 提交即可回到 `@v4`。不需要动锁文件。
