# Remove zot-bridge plugin and desktop write backend

## Goal

彻底移除自研 `zot-bridge` XPI 插件及其整条 desktop 写后端链路,merge/dedupe 收敛为
Web API 单后端。移除后,zot 的本机面只剩:直连 SQLite 只读、`/api/` 只读探测、
`/connector/` 导入(由子任务 connector-local-write 提供)。

## Requirements

### R1 删除清单(代码)

- `plugins/zot-bridge/`(bootstrap.js、manifest.json、tests)整目录删除。
- `src/zot-cli/src/commands/bridge.rs`:整文件删除(XPI 打包、setup/pair/status/revoke)。
- `src/zot-desktop`:删除 bridge client(pair/token/merge、BridgeEnvelope、协议校验),
  保留/改造为 connector + local API 探测 client(`probe_local_http` 继续供 doctor 使用)。
- `src/zot-cli/src/commands/item/merge.rs`:删除 `DesktopMergeWriter`、`MergeWriterPlan::Desktop`、
  `SecretPlanToken`;`selected_merge_writer` 直接返回 `WebMergeWriter`。
- `zot-core`:删除 `WriteBackend` enum、`DesktopBridgeConfig`、`bridge_connection_id`、
  `ZotError::DesktopBridge`(或收窄为 connector 错误,design.md 定)、config 中
  `write_backend` / `desktop_bridge` 字段及其 profile 级拷贝逻辑。
- CLI args:删除 `--write-backend` global flag、`BridgeCommand` 及子命令注册。
- `config.rs`:删除 `ConfigKeyArg::WriteBackend`(`config.rs:245`)——这是 `config set`
  可写键,不删会残留 `zot config set write_backend`。
- doctor:删除 `capabilities.desktop_write`、`selected_write_backend`、bridge health/status 探测与文案。
- `justfile`:删除 `plugin-test` 与 `xpi-check` target(删 `plugins/` 后即变坏命令)。

### R2 envelope schema(用户已定:全删字段,api_version 保持 1)

- model 4 处 `write_backend`、doctor `selected_write_backend` / `desktop_write` 全删;
  `meta.api_version` 不 bump(仍为 1)。
- 必须同步 `docs/agents/limits.md`:删去展示这些字段的 envelope 示例,软化
  `api_version==1` 的截止标记措辞(改为「字段可能随小版本增删,以 CHANGELOG 为准」),
  否则删字段会与该文档自相矛盾。CHANGELOG 说明字段在 api_version 不变下移除。

### R3 配置兼容

- 旧 config 文件中残留 `zotero.write_backend` / `zotero.desktop_bridge`(含 profile 内)时:
  加载不得报错(serde 忽略未知字段),`zot config show` 不再展示;`zot doctor` 检测到
  **`desktop_bridge` 段或 `write_backend` 键**(根级+profile,不解引用 token 值)时输出一次性
  迁移提示。注意:仅 `write_backend="desktop"` 而无 token 的旧配置也要覆盖,只查 token 会漏。
- 不自动改写用户 config 文件。

### R4 行为收敛

- `item merge` / `library duplicates-merge` / `library dedupe` 只走 Web API;
  无凭据时报既有 `web-credentials` 类错误,hint 不再提及 bridge。
- 错误码 `bridge-*` 全部消失;帮助文本、`--help`、错误 hint 中无 bridge 字样。

### R5 文档与 spec

- README.md / README.zh-CN.md:删除 bridge 章节,补 connector 导入说明。
- CHANGELOG.md:记录 breaking change(bridge 移除、`--write-backend` 移除、
  `config set write_backend` 移除、本机 merge 改走 Web API、envelope 字段移除)。
- **双语 VitePress docs(dist/ 不手改)**:逐页清理 `docs/cli/{config,item,library,troubleshooting,overview}.md`、
  `docs/guide/getting-started.md`、`docs/skills/{routing,safety,overview}.md`、`docs/agents/limits.md`
  及各自 `docs/en/` 镜像中的 `write_backend` / `pair` / `desktop_write` / bridge 引用;
  中英一致。`npm --prefix docs run build` 必须通过。
- `.trellis/spec/zot-cli/backend/desktop-bridge.md`:删除或改写为 connector 说明;
  `merge-dedupe.md`、`index.md` 同步;`zot-core/backend` spec 中 config 相关段落同步。

## Acceptance Criteria

- [ ] 全仓 `grep -ri "zot-bridge\|bridge" src plugins` 仅剩合理残留(如 CHANGELOG 历史记录);`plugins/` 目录不存在
- [ ] `zot bridge ...`、`zot config set write_backend` 不存在;`zot --help` / 子命令 help 无 bridge/write-backend 字样
- [ ] `just --list` 无 `plugin-test` / `xpi-check`
- [ ] `item merge` preview + `--confirm` 在配好 Web 凭据时端到端可用;无凭据时错误信息只指向 Web API 配置
- [ ] envelope 不再含 `write_backend` / `selected_write_backend` / `desktop_write`;`api_version` 仍为 1;`limits.md` 无矛盾
- [ ] 携带旧 `desktop_bridge` 段**或**仅 `write_backend="desktop"` 的 config 可正常加载并得到迁移提示
- [ ] 最终门:`just ci` 全绿 + `npm --prefix docs run build` 通过 + `git diff --check` 干净;删除路径涉及的测试同步清理
- [ ] README / CHANGELOG / 双语 docs / spec 更新完成

## Notes

- 依赖:子任务 `07-18-connector-local-write` 先合入(doctor 能力位一次性重排,
  避免中间态出现"既无 desktop_write 又无 connector_write"的空窗)。
- 风险:曾配对用户升级后 desktop merge 静默消失 —— 通过 CHANGELOG breaking 说明
  + doctor 迁移提示覆盖,不做运行时自动迁移。
