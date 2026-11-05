# Dependabot PR #17–#24 审查证据

记录日期：2026-10-12。只读来源：本仓库 `dev`（`79934a1`）、`main`（`5b368d0`）、`gh pr view` / `gh run view`。未改产品代码，未合并 PR。

## 分支

- 默认分支是 `main`。8 个 PR 的 base 都是 `main`，作者都是 Dependabot。
- `main...dev` 左侧 3 个提交只是把旧 `dev` 合并进 `main` 的 merge commit（#13、#14、#15），不是独立功能。
- `dev` 比 `main` 超前 24 个提交。本地 `dev` 还比 `origin/dev` 超前 6 个提交，尚未推送。
- `dev` 的 `Cargo.lock` 里 `rustls` 已是 `0.23.45`（`a2120b3`）。`main` 上的 PR CI 仍扫到 `rustls` 0.23.38。

## 当前 dev 锁版本

| crate | `dev` 锁版本 | workspace 要求 |
| --- | --- | --- |
| clap | 4.6.1 | `4.6.1`（caret，允许 4.6.x） |
| thiserror | 2.0.18（另有传递 1.0.69） | `2.0.18` |
| dirs | 6.0.0 | `6.0.0`（不允许 7） |
| tar | 0.4.45 | `0.4.44`（caret，允许 0.4.46） |
| http | 1.4.0 | `1.3.1`（caret；没有任何 crate 写 `http.workspace = true`） |
| rustls | 0.23.45 | 传递依赖 |

## CI 时间

- #17–#21 的检查完成于 2026-09-13，全部成功。RUSTSEC-2026-0285 的公告日期是 2026-09-14，所以这五次绿色安全检查早于公告，不能当作当前 `main` 仍然干净。
- #22–#24 的检查完成于 2026-10-01。

## 各 PR

### #24 `dtolnay/rust-toolchain` 1.85.0 → 1.120.0

- 不是仓库里的 `rust-toolchain` 文件。仓库没有这个文件。
- diff 只有 `.github/workflows/ci.yml` 的 MSRV job：`dtolnay/rust-toolchain@1.85.0` → `@1.120.0`。
- `stable` 与 `nightly-2026-07-01` 两处引用没变。
- 1.85 是故意的 MSRV：`Cargo.toml` 的 `rust-version = "1.85"`，双语 README 的 MSRV 徽章，`AGENTS.md`，以及 CI job 名 “Rust 1.85.0 / locked workspace”。该 job 运行 `cargo check --workspace --locked`。
- MSRV job 失败原因：`could not download nonexistent rust version 1.120.0-x86_64-unknown-linux-gnu`，rustup 下载返回 404。run `36905679068`，job `110515494330`，2026-10-01。
- 同一 run 的 security job 也失败，原因是 `main` 上的 rustls 0.23.38 / RUSTSEC-2026-0285，不是这一行 toolchain 引用引起的。

### #22 `actions/checkout` 4 → 7

- 改 `.github/workflows/ci.yml` 六处和 `deploy-docs.yml` 一处，都是 `@v4` → `@v7`。
- 除 security 外的 job 成功，包括 docs / VitePress build。
- security 失败同样是 rustls 0.23.38 / RUSTSEC-2026-0285（run `36905661095`）。checkout 升级不改 `Cargo.lock`。
- #24 的 MSRV 日志里，`actions/checkout@v4` 触发了 Node.js 20 deprecation 警告。

### #23 `actions/setup-node` 4 → 7

- 只改 `ci.yml` 与 `deploy-docs.yml` 的 `actions/setup-node@v4` → `@v7`。`node-version: 20` 没变。
- docs job 成功。security 失败原因与 #22 相同（run `36905671325`）。

### #19 `dirs` 6.0.0 → 7.0.0

- 改 `Cargo.toml` 与 `Cargo.lock`。2026-09-13 的 CI 全绿，含 Windows。
- crates.io：6.0.0 发布于 2025-01-12，7.0.0 发布于 2026-09-05。Changelog 来自 crates.io readme。
- 7 的 breaking change 只有 Windows 上的 `preference_dir`：从 `LocalAppData` 改为 `RoamingAppData`。`config_dir`、`cache_dir`、`home_dir` 不在这条变更里。
- 本仓库调用点：
  - `src/zot-core/src/config.rs`：`dirs::config_dir()`、`dirs::home_dir()`
  - `src/zot-local/src/pdf.rs`：`dirs::cache_dir()`
- 没有 `preference_dir` 调用。

### #17 clap 4.6.1 → 4.6.6

- 只改 `Cargo.lock`。2026-09-13 CI 全绿。
- 发布说明里的 4.6.6 变更是新增 `Command::get_overridden_usage`。本仓库用 derive / `Parser` / `CommandFactory`，不依赖这个新 API。

### #18 thiserror 2.0.18 → 2.0.20

- 只改 `Cargo.lock`。2026-09-13 CI 全绿，含 Rust 1.85 job。
- 2.0.19 换到 syn 3；2.0.20 抑制生成代码里的 `redundant_field_names`。直接使用点在 `src/zot-core/src/error.rs` 的 `thiserror::Error`。锁里还有传递的 thiserror 1.0.69，这条 PR 不应动它。

### #20 tar 0.4.45 → 0.4.46

- 只改 `Cargo.lock`。2026-09-13 CI 全绿。
- 0.4.46 的发布说明标明安全修复：PAX header desync，GHSA-3cv2-h65g-fgmm。
- 使用点在 `src/zot-local/src/pdf.rs`：`tar::Archive`、`tar::Builder`、GNU header、symlink entry。现有测试会造 tar。

### #21 http 1.4.0 → 1.5.0

- 只改 `Cargo.lock`。2026-09-13 CI 全绿。
- 1.5.0 发布说明包含 QUERY method、URI builder 允许空 path、更快的校验，以及 PathAndQuery 最大长度限制。
- `http` 出现在 workspace.dependencies，但没有成员 crate 声明 `http.workspace = true`。锁里的 1.4.0 是传递依赖。审查对象是这条传递升级，不是新增直接依赖。

## 兼容性复核（2026-10-12）

落地决定：接受的改动进入本地 `dev`，不合并这 8 个 PR 到 `main`。#24 关闭。其余 PR 在对应提交写到本地 `dev` 之后关闭。不推送 `origin/dev`。

| PR | 结论 | 依据 |
| --- | --- | --- |
| #24 | 拒绝 | MSRV 钉扎；1.120.0 在 rustup 上 404 |
| #22 | 接受 `@v7` | v5 起要求 runner ≥ 2.327.1，2026-10-01 的 GitHub-hosted job 已通过。v6 把凭据放到单独文件；该 PR 的 stable job 含 `git diff --exit-code` 且成功。v7 只阻止 `pull_request_target` / `workflow_run` 检出 fork PR，本仓库没有这两种事件 |
| #23 | 接受 `@v7` | v5 的自动缓存只在 `package.json` 有 `packageManager` 时生效；`docs/package.json` 没有该字段。workflow 仍显式 `cache: npm`、`node-version: 20`。docs job 已通过。v6 把自动缓存限制为 npm。v7 没有改变这两个输入 |
| #19 | 接受 7.0.0 | 见 `../10-12-review-dirs-major/research/dirs-7-paths.md`。唯一源码差异是 Windows `preference_dir`，本仓库未调用 |
| #17 | 接受 4.6.6 | 新增 API，现有 derive / Parser / CommandFactory 不需要改。2026-09-13 的 Rust 1.85 job 通过 |
| #18 | 接受 2.0.20 | 同一 MSRV job 通过。只更新名为 2.0.18 的那一份，保留传递的 1.0.69 |
| #20 | 接受 0.4.46 | GHSA-3cv2-h65g-fgmm。使用点在 `src/zot-local/src/pdf.rs` |
| #21 | 接受锁文件中的 1.5.0 | 传递依赖，经 reqwest / hyper 使用。1.4.1 到 1.5.0 的 URI 收紧包括 PathAndQuery 最长 `u16::MAX - 1` 字节，以及拒绝不以 `/` 开头的 PathAndQuery。本仓库用 `url::Url` 解析普通地址，不直接构造 `http::Uri`。不把未使用的 workspace 键改成直接依赖 |
