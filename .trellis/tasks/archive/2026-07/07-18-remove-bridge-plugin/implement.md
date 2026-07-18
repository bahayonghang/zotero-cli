# Implement — remove zot-bridge plugin and desktop write backend

前置:子任务 `07-18-connector-local-write` 已合入(`ZotError::Connector` 与
`connector_write` 能力位已存在)。按层自外向内删,每步保持可编译。

## Checklist

1. [x] `zot-cli`:删 `commands/bridge.rs`、`BridgeCommand` args 与路由注册、
       `--write-backend` global flag 及 `ctx.write_backend()` 消费点。
       → verify: `cargo build -p zot-cli`(预期报出全部下游断点,逐个清)
2. [x] `merge.rs`:删 `DesktopMergeWriter` / `MergeWriterPlan::Desktop` /
       `SecretPlanToken` / desktop 转换函数;`selected_merge_writer` 直返
       `WebMergeWriter`;同步 `library_dedupe.rs` 受影响引用与两文件的单测。
       → verify: `cargo test -p zot-cli merge`(注意:cargo test 只接一个 filter,
       `merge dedupe` 会报 unexpected argument;dedupe 用例并入 `just test` 全跑)
3. [x] `config.rs`:删 `ConfigKeyArg::WriteBackend`(`config.rs:245`)分支及其 arg
       解析/帮助文本——这是可设置键,不删会残留 `config set write_backend`。
       → verify: `cargo run -q -p zot-cli -- config set --help`(无 write_backend)
4. [x] `doctor.rs`:删 `desktop_write` 能力、bridge 探测、`selected_write_backend`;
       加 config 残留检测(toml::Value 判 `desktop_bridge` 段**或** `write_backend` 键
       存在,根级+profile,不解引用 token 值)与迁移 hint;更新单测(含两条用例:
       「旧 config 带 desktop_bridge 段」与「仅 write_backend="desktop" 无 token」均出 hint)。
       → verify: `cargo test -p zot-cli doctor`
4. [x] `zot-desktop`:删 bridge client 方法与 model 类型、`BRIDGE_BASE_URL`;
       `probe_local_http`/`LocalHttpStatus` 并入 connector 模块;清理 lib.rs 导出与
       全部 bridge 单测。
       → verify: `cargo test -p zot-desktop`
5. [x] `zot-core`:删 `WriteBackend`、`DesktopBridgeConfig`、`bridge_connection_id`、
       `ZotError::DesktopBridge`、model 4 处 `write_backend` 字段、config 字段与
       `set_desktop_bridge*` / `clear_desktop_bridge` / profile 拷贝;更新单测。
       → verify: `cargo test -p zot-core`
6. [x] 删 `plugins/zot-bridge/` 整目录;移除 `justfile` 的 `plugin-test` 与 `xpi-check`
       target;移除 workspace + zot-cli 的 `zip` 依赖(已核实仅 bridge.rs 用);
       **保留 `opener`**(`util.rs` 的 `item open` 共用)。
       → verify: `cargo build --workspace`(无 unused-dep 警告);`just --list` 无 plugin-test/xpi-check
7. [x] envelope schema 收敛(决策:全删字段,api_version 保持 1):
       删 model 4 处 `write_backend`、doctor `selected_write_backend` 与 `desktop_write`;
       **不改 `api_version` 值(仍为 1)**;改 `docs/agents/limits.md`——删展示这些字段的
       envelope 示例、软化 `api_version==1` 截止标记措辞(见 design.md 输出契约小节)。
       断言 `api_version==1` 的现有测试保持不变(值没变)。
       → verify: `cargo test -p zot-core`、`cargo test -p zot-cli doctor`
8. [x] 全仓文案清零:grep `bridge`、`write-backend`、`desktop_write`、`pair` 于
       `src/`,清 help 文本、错误 hint、注释残留。
       → verify: `grep -rin "bridge" src/ | grep -v connector` 为空
9. [x] 文档:README.md / README.zh-CN.md 重写写入路径章节;CHANGELOG.md 记
       breaking(design.md 列表 + 迁移动作);双语 VitePress docs 按 design.md 清单逐页清理。
       → verify: `npm --prefix docs run build` 通过;人工过两个 README 写入小节
10. [x] spec:删 `.trellis/spec/zot-cli/backend/desktop-bridge.md`,改 `index.md`、
        `merge-dedupe.md`、`zot-core/backend` config 段(走 trellis-update-spec 流程)。
        → verify: spec index 无死链
11. [x] 全量门(与本仓库 CI 一致)。
        → verify: `just ci` 全绿 + `npm --prefix docs run build` 通过 + `git diff --check`(无空白错误)
12. [ ] 手工端到端(实机):
    - [ ] 配好 Web 凭据:`item merge` preview → `--confirm` 成功,输出 envelope 符合选定 schema 方案。当前实机无 Web 凭据,且未获真实库写入授权,未执行。
    - [x] 无凭据:merge 报 `write-credentials`,hint 无 bridge 字样。
    - [x] 实机旧 config 同时残留 `desktop_bridge` / `write_backend="desktop"` 时,`cargo run -q -p zot-cli -- --json doctor` 正常并只输出一条迁移 hint;root/profile 两类键另有单测覆盖。
    - [x] `zot bridge status` / `zot config set write-backend desktop` → clap unknown。

## Validation Evidence

- `just ci`:通过(fmt/check/clippy `-D warnings`/217 Rust tests/skills-check)。
- `npm --prefix docs run build`:通过。
- `git diff --check`:通过。
- 实机 doctor:`connector_write.available=true`,无 `desktop_write` / `selected_write_backend`,`meta.api_version=1`。
- 缺失证据:未执行带真实 Web 凭据的 `item merge --confirm`;不将其记为通过。

## Review gate

- 步骤 2 后:dedupe 的 preview/confirm 同后端约束逻辑仍成立(现在恒为 Web,
  相关防御代码可简化但语义不得反转)。
- 步骤 7 后:检查 `--json` envelope 快照类单测是否全部更新,禁止留 skip;
  `api_version` 断言与选定方案一致。

## Rollback

- 单分支实现,合并前整分支可弃;合并后回滚 = revert merge commit。
- 无配置写入、无数据迁移,回滚无残留。
