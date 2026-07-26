# P2: 工程化与文档基线

## Goal

关闭审计 QW-07/M-07 的工程化缺口，使本地真实门禁与 GitHub CI 使用同一套纯检查契约，
在三平台和声明的 Rust 1.85 MSRV 上验证锁文件，并把依赖、lint、版本和 agent 控制面
文档纳入可执行约束。

## Background

- 父任务 `07-26-audit-remediation` 将 CI matrix/`--locked`/MSRV/audit、四个 crate
  lint 继承、justfile 漂移、`AGENTS.md`、1.0.0 CHANGELOG 和未使用 `rmcp` 映射到本任务。
- 审计报告 `zotero-cli-code-audit-2026-07-25.md:153-168` 的六项证据在当前 `dev`
  仍成立：`.github/workflows/ci.yml` 只有 Ubuntu stable；四个 member manifest 缺
  `[lints] workspace = true`；`just ci` 会先改写 skill；`AGENTS.md` 写成 4 crate 且称
  无 CI；workspace 版本为 1.0.0 而 CHANGELOG 停在 0.6.0；`rmcp` 仅在 workspace
  dependencies 声明，未被任何 crate 消费。
- 当前 `cargo +1.85.0 check --workspace --locked` 失败：`icu_* 2.2` 要求 1.86，
  `image 0.25.10` 和 `libloading 0.9` 要求 1.88。`pdfium-render` 默认 image feature
  引入未使用的 image 栈，且其宽松 `libloading = "0"` 解析到了不兼容版本。
- 当前 `cargo audit` 发现 4 个漏洞和 1 个 unsound advisory：`quick-xml 0.38.4`
  两项 DoS、`quinn-proto 0.11.14` 内存耗尽、`rustls-webpki 0.103.12` panic，以及
  `anyhow 1.0.102` unsound；新增 audit gate 前必须升级到已修补且兼容 MSRV 的版本。
- 启用 workspace lint 会暴露 `zot-core`/`zot-local` 测试中的 39 个 `unwrap()`，以及
  `zot-local/pdf.rs` 测试对 edition-2024 `env::set_var/remove_var` 的两处 unsafe。

## Requirements

### R1 纯本地门禁

- `version-sync` 和 skill mirror 同步保持显式写操作；`version-check`、`skills-check`、
  `ci-check` 与兼容入口 `just ci` 必须只读，不得先修复漂移再宣称通过。
- Rust check/clippy/test/version guard/build 的可复现路径使用 `--locked`；`just ci`
  继续按 fmt、check、clippy、test、skill/版本检查顺序覆盖仓库真实门禁。
- canonical skill 与镜像漂移时 `skills-check` 必须失败；安装流程仍可显式同步镜像。

### R2 GitHub CI

- stable job 以 `ubuntu-latest`、`windows-latest`、`macos-latest` matrix 直接执行
  `just ci-check`，并在门禁后用 `git diff --exit-code` 证明检查未改写仓库。
- 单独 MSRV job 使用 Rust 1.85.0 和 `Cargo.lock` 执行 workspace check。
- 单独依赖安全/卫生 job 执行 `cargo audit`、`cargo deny check`、`cargo machete`；
  `cargo udeps` 使用固定 nightly job，避免把 nightly 引入正常 build/test 路径。
- 所有依赖安装和 cargo 构建命令应可复现；本任务不新增发布、签名、SBOM 或 provenance。

### R3 MSRV 与依赖安全

- 保持 workspace `rust-version = "1.85"`，移除 `rmcp` workspace 声明直到 MCP 实现。
- 关闭未使用的 `pdfium-render` image feature，并将宽松传递依赖锁到 Rust 1.85
  可编译版本；不得削弱已归档 Pdfium 可信加载、校验下载或现有提取能力。
- 升级 audit 报告的五个受影响依赖到已修补、Rust 1.85 可用的版本；`cargo audit`
  和 `cargo deny check advisories` 不得依赖无依据 ignore 才通过。
- `deny.toml` 显式允许仓库当前依赖实际使用的开源许可证、拒绝未知 registry/git source，
  不把 duplicate-version warnings 升格为本任务的全量依赖重构。

### R4 Workspace lint

- 五个 member crate 均显式继承 `[workspace.lints]`；guard test 从根 workspace members
  解析清单并逐一断言，避免未来新增 crate 漏继承。
- 处理启用后真实暴露的 lint，不使用 crate 级 allow 绕过。测试中的成功路径用有信息的
  `expect`；环境变量测试改为无 unsafe 的可注入纯 helper，不削弱 `unsafe_code = forbid`。

### R5 版本与控制面文档

- `AGENTS.md` 准确列出 5 个 crate、纯 `just ci` 契约、GitHub CI 和新增依赖门禁。
- `CHANGELOG.md` 增加 1.0.0 条目，覆盖本轮安全/可靠性整改、破坏性或操作迁移信息，
  并提示曾使用附件上传的用户轮换 Zotero API key。
- workspace guard 从根版本生成 `## [<version>]` 断言，并继续保护内部依赖集中声明；
  不引入从 Cargo metadata 生成 AGENTS/CHANGELOG 的新生成器。

### R6 范围与兼容性

- 不改变 CLI 命令、JSON envelope、Zotero 本地只读/Web 写入边界或 Pdfium 功能语义。
- 长期 release engineering（artifact install smoke、SBOM、签名、provenance）、
  application layer、observability 和 MCP 实现保持父任务范围外。

## Acceptance Criteria

- [x] `just ci` 与 `just ci-check` 在 dirty implementation tree 上只执行检查且不改写文件；
      人为制造 canonical/mirror drift 时 `just skills-check` 失败。
- [x] workflow 明确包含三平台 stable matrix、Rust 1.85 MSRV、`--locked`、audit、deny、
      machete、udeps 和检查后的 `git diff --exit-code`。
- [x] `cargo +1.85.0 check --workspace --locked` 通过，且 Pdfium 相关聚焦测试保持通过。
- [x] `cargo audit` 无 vulnerability/unsound finding；`cargo deny check` 通过，安全策略无
      无依据 advisory ignore；`cargo machete` 与 `cargo udeps` 通过或由等价 CI 命令验证。
- [x] 五个 crate 均继承 workspace lint，workspace/all-targets locked clippy 在
      `-D warnings` 下通过，guard test 对缺失 lint inheritance 会失败。
- [x] workspace version guard 断言 1.0.0 CHANGELOG heading；AGENTS 的 crate/CI 说明与
      `Cargo.toml`、`justfile`、workflow 一致。
- [x] `CHANGELOG.md` 的 1.0.0 条目包含安全整改和附件上传 API key 轮换提示。
- [x] `task.py validate`、`git diff --check`、聚焦测试与最终真实门禁 `just ci` 全部通过；
      可复用工程契约同步到 `.trellis/spec/`。

## Out Of Scope

- 发布 artifact 安装 smoke、release workflow、SBOM、签名、provenance、自动发版或 tag。
- 消除所有 duplicate dependencies、升级全部依赖到最新、重写 workspace 架构。
- 实现 MCP/rmcp、增加 application/use-case layer 或长期 daemon observability。
- 通过提升 MSRV 回避锁文件兼容问题。
