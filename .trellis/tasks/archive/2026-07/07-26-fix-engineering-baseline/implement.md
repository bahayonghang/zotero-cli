# Implementation Plan: 工程化与文档基线

## 1. 可执行契约

- [x] 扩展 `workspace_version_guard`：遍历五个 member lint inheritance，并校验当前版本的
      CHANGELOG heading；先证明旧状态失败。
- [x] 分离 `skills-sync`/`skills-check` 与 `version-sync`/`version-check`，新增纯
      `ci-check` 并让 `ci` 保持兼容别名；cargo gates 加 `--locked`。
- [x] 更新 skill mirror tests/规范，证明 check 不修复 drift。

## 2. Manifest、MSRV 与 lint

- [x] 删除未消费 `rmcp`，关闭未使用的 Pdfium image feature。
- [x] 为 `zot-core`、`zot-local`、`zot-desktop`、`zot-remote` 添加 workspace lint inheritance。
- [x] 将测试 `unwrap()` 改为有上下文的 `expect()`；用可注入 helper 移除 Pdfium env test unsafe。
- [x] 更新并精确收敛 `Cargo.lock` 到 Rust 1.85 兼容、已修补版本。
- [x] 运行 `cargo +1.85.0 check --workspace --locked` 与 Pdfium/全 workspace 聚焦测试。

## 3. 依赖审计

- [x] 新增 `deny.toml` 的 advisory/license/source/bans 策略。
- [x] 修复 `cargo audit` 当前 4 vulnerability + 1 unsound finding，不添加 advisory ignore。
- [x] 运行 `cargo deny check`、`cargo machete`、`cargo udeps`，处理真实直接依赖问题。

## 4. CI 与文档

- [x] 将 CI 改为三平台 stable `just ci-check` matrix，并检查 clean diff。
- [x] 增加 Rust 1.85 MSRV、audit/deny、machete/固定 nightly udeps jobs。
- [x] 更新 `AGENTS.md` 的五 crate、CI、门禁事实。
- [x] 增加 CHANGELOG 1.0.0 安全/可靠性/迁移条目和附件上传 API key 轮换提醒。
- [x] 将纯检查、MSRV、lint inheritance、依赖审计契约写入 `.trellis/spec/`。

## 5. 验证与收尾

- [x] 运行 `cargo fmt --all` 和 guard/Pdfium/各 crate 聚焦测试。
- [x] 运行 `cargo clippy --workspace --all-targets --locked -- -D warnings`。
- [x] 运行 `cargo audit`、`cargo deny check`、`cargo machete`、`cargo udeps`。
- [x] 运行 `python ./.trellis/scripts/task.py validate ...`、`git diff --check`、`zot --json doctor`。
- [x] 运行真实最终门禁 `just ci`，逐项勾选 PRD acceptance，按中文 emoji `[AI]`
      原子提交、归档并记录 journal。

## Risk / Rollback Points

- 依赖解析以 Rust 1.85 实际 check 为准；不要只依赖 crate metadata 的 `rust-version`。
- 关闭 Pdfium image feature 前确认源码没有 image API；可信加载/下载实现不得改动。
- `git diff --exit-code` 只放 clean checkout workflow，避免本地 dirty task 无条件失败。
- cargo-deny 的 license allow list 必须来自实际依赖证据；不因重复版本 warning 扩大清理范围。
- udeps 使用 nightly 是分析工具约束，不得把 workspace toolchain/MSRV 改为 nightly。
