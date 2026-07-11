# Implement: Zotero 本机桥接基础

## Pre-Development

- [x] 本子任务获批并 `task.py start` 后运行 `trellis-before-dev`。
- [x] 读取 zot-core/zot-cli backend spec、shared guides 和 task research。
- [x] 运行 `cargo run -q -p zot-cli -- --json doctor` 作为环境基线，之后保持该调用路径。

## Stage 1: Protocol And Rust Client

- [x] workspace 增加 `zot-desktop` crate 和 `ZotError::DesktopBridge`。
- [x] 定义 v1 health/pair/status/revoke DTO、response meta、capability 和 error types。
- [x] 实现 loopback-only reqwest client、auth、timeout、version negotiation、redacted Debug。
- [x] fake server 测成功、401、404、timeout、invalid JSON、protocol mismatch 和 replay。
- 验证：`cargo test -p zot-desktop -p zot-core`

## Stage 2: Config And CLI Surface

- [x] 增加 `WriteBackend`、`DesktopBridgeConfig`、root/profile materialization 和 migration tests。
- [x] global `--write-backend`、backend precedence、config view redaction。
- [x] 增加 `bridge setup|pair|status|revoke` args、dispatch、JSON/human output。
- [x] setup deterministic XPI writer 与 `just xpi-check`。
- 验证：`cargo test -p zot-cli -- config bridge`; `just xpi-check`

## Stage 3: Zotero Plugin

- [x] manifest/bootstrap/lifecycle、endpoint registration/unregistration。
- [x] schema、Host/Origin/body/method guard、pairing、token hash、replay cache。
- [x] Tools 菜单 pairing dialog 和 Reset Authorization。
- [x] health/status/revoke capability response；第一阶段不注册 mutation endpoint。
- [x] JS test harness 或 mock tests 覆盖纯协议和 auth 函数。
- 验证：plugin unit tests；XPI archive check。

## Stage 4: Doctor And Integration

- [x] Local HTTP probe 与 desktop health probe 分开。
- [x] doctor 新 capabilities，保留 web `write_credentials` 兼容字段。
- [x] 错误 hint 覆盖 Zotero 未运行、plugin 未安装、未配对和不兼容。
- [x] 端到端 fake client + CLI JSON tests。
- 验证：`cargo test -p zot-cli -- doctor`; `just ci`

## Zotero Smoke Gate

在隔离测试 profile：

- [x] setup 生成 XPI。
- [x] 手动安装 XPI 并重启 Zotero。
- [x] 首次 code 五分钟后失效；有效 code 只可用一次。
- [x] pair 后 status 可用，错误 token 401，revoke 后 token 失效。
- [x] browser Origin 请求被拒；shutdown/uninstall 后 endpoint 404。
- [x] 检查 Zotero 日志和 CLI 输出无 code/token。

Smoke evidence (2026-07-11, no secrets recorded): Zotero 9.0.6 loaded plugin
0.6.0/protocol 1; reused code returned `bridge-pair-expired`; a fresh code
expired after a measured 306 seconds without invalidating the existing token;
real revoke cleared both sides and re-pair restored access; disabling the
plugin produced HTTP 404/`bridge-not-installed` while Local HTTP stayed
available; re-enable restored `desktop_write.available = true`.

## Risk And Rollback

- 风险文件：`src/zot-core/src/config.rs`、`src/zot-cli/src/context.rs`、doctor 输出。
- 每个 stage 是独立回滚点；不在本任务引入 mutation endpoint。
- plugin 崩溃或协议异常时卸载 XPI并保留 CLI 配置供诊断；不自动切换 web。
- 完成本任务后先归档/验收，再启动 `07-11-local-merge-dedupe`。
