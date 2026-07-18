# Design — remove zot-bridge plugin and desktop write backend

## 删除边界

| 层 | 删除 | 保留/改造 |
| --- | --- | --- |
| `plugins/zot-bridge/` | 整目录(bootstrap.js、manifest.json、tests) | — |
| `zot-cli/commands/bridge.rs` | 整文件(含 XPI 打包与 zip 依赖使用点) | — |
| `zot-cli` cli/args | `BridgeCommand` 树、`--write-backend` global flag | — |
| `zot-cli/commands/item/merge.rs` | `DesktopMergeWriter`、`MergeWriterPlan::Desktop`、`SecretPlanToken`、`desktop_preview/apply_result` | `selected_merge_writer` 收敛为直接构造 `WebMergeWriter` |
| `zot-cli/commands/doctor.rs` | `desktop_write` 能力、bridge health/status 探测与 label | `local_http_read`、connector_write(上任务已加) |
| `zot-desktop` | `client.rs` bridge 方法(health/pair/status/revoke/merge_*)、`model.rs` bridge 类型、`BRIDGE_BASE_URL`、协议校验 | `probe_local_http` + `LocalHttpStatus` 迁入 connector 模块;crate 名不改(避免无谓 churn) |
| `zot-core` | `WriteBackend`、`DesktopBridgeConfig`、`bridge_connection_id`、`ZotError::DesktopBridge`、config `write_backend`/`desktop_bridge` 字段及 profile 拷贝、`set_desktop_bridge*`/`clear_desktop_bridge` | `ZotError::Connector`(上任务已加)承接本机 HTTP 错误 |
| `zot-cli/commands/config.rs` | `ConfigKeyArg::WriteBackend` 枚举分支(`config.rs:245` 的可设置键 `write_backend`)及其 arg 解析/帮助 | 其余 config key 不动 |
| `justfile` | `plugin-test`、`xpi-check` 两个 target(删 `plugins/` 后立即变坏命令) | `ci` target 不引用它们,无需改;其余不动 |
| workspace + `src/zot-cli` `Cargo.toml` | `zip` 依赖(已核实仅 bridge.rs 使用:ZipWriter/ZipArchive/SimpleFileOptions) | `opener` 必须保留(`util.rs:55` 的 `item open` 共用) |

## 输出契约变更(breaking,写进 CHANGELOG)

**决策(用户已定):全删字段,`api_version` 保持 1,不升 v2。**

- `MergePreview` / `MergeApplyResult` 等 4 处模型(`zot-core/src/model.rs:366,385,444,463`)
  的 `write_backend` 字段**删除**,不保留常量 "web"。
- `doctor` JSON:`selected_write_backend` 字段删除;`capabilities.desktop_write` 删除。
- `meta.api_version` **维持 1**(不 bump)。
- **必须同步 `docs/agents/limits.md`**:该文件把 `api_version==1` 定为 0.5.0 契约的
  截止标记(`limits.md:80`)。因为本次在 api_version 不变的情况下改了 schema,需要:
  1. 删除该文档中任何仍展示 `write_backend` / `selected_write_backend` / `desktop_write`
     的 envelope 示例;
  2. 调整截止标记措辞,改为「字段可能随小版本增删,以 CHANGELOG 为准」之类,避免文档
     继续声称 api_version==1 等同稳定 schema —— 否则删字段会让 limits.md 自相矛盾。
- CHANGELOG 明记:这些字段在 api_version 不变的前提下移除(消费方若硬编码这些字段需按
  CHANGELOG 调整,不能只看 api_version)。
- CLI:`zot bridge ...`、`--write-backend`、`config set write_backend` 消失(clap 报 unknown;
  无 alias、无废弃期,0.x 版本可接受,CHANGELOG 标 breaking)。

## 配置兼容策略

- config 反序列化未启用 `deny_unknown_fields`(已核实),字段删除后旧 config 中的
  `zotero.write_backend` / `zotero.desktop_bridge`(含 profile 内)会被 serde 静默忽略,
  加载不报错 —— R2 的"不崩溃"由此免费获得。
- 迁移提示:`doctor` 读 config 文件为 `toml::Value`,检测**下列任一**存在(根级或任一
  profile)即输出一次性 hint:
  - `desktop_bridge` 段(不解引用 token 值,只判键存在,避免读出/打印敏感值);
  - `write_backend` 键(尤其 `= "desktop"` 的旧配置——它没有 token,但仍是 desktop 意图,
    只查 `desktop_bridge.token` 会漏掉这批用户)。
  hint 文案:「Zot Bridge 已移除:可删除配置中的 desktop_bridge / write_backend 段,
  并在 Zotero 插件管理器中卸载 Zot Bridge 插件」。不自动改写 config。

## merge/dedupe 收敛

- `selected_merge_writer(ctx)` → 不再匹配后端,直接 `WebMergeWriter::new(ctx.remote()?)`;
  函数签名与 `MergeWriter` trait 保留(dedupe 的 `WriterGroupMerger` 复用它,改动最小)。
- 无 Web 凭据时走 `ctx.remote()` 既有 credential 错误;全仓 hint 文案 grep `bridge`
  清零(错误码 `bridge-*` 随 `ZotError::DesktopBridge` 变体一起消失)。

## 文档与 spec 同步

- README.md / README.zh-CN.md:删 bridge 安装/配对章节与 capability 描述,
  写入路径改述为「connector 导入(本机、仅新增) + Web API(全部 mutation)」。
- CHANGELOG.md:breaking —— bridge 移除、`--write-backend` 移除、`config set write_backend`
  移除、merge/dedupe 仅 Web、envelope schema 变更(见下节);附旧用户迁移动作
  (卸载插件、清 config 段、配 Web 凭据)。
- **VitePress 双语 docs(必须显式纳入,dist/ 是构建产物不手改)**——以下源文件均引用
  `write_backend` / `pair` / `desktop_write` / bridge,需逐页清理并保持中英一致:
  - `docs/cli/config.md` + `docs/en/cli/config.md`
  - `docs/cli/item.md` + `docs/en/cli/item.md`
  - `docs/cli/library.md` + `docs/en/cli/library.md`
  - `docs/cli/troubleshooting.md` + `docs/en/cli/troubleshooting.md`
  - `docs/cli/overview.md` + `docs/en/cli/overview.md`
  - `docs/guide/getting-started.md` + `docs/en/guide/getting-started.md`
  - `docs/skills/routing.md`、`docs/skills/safety.md`、`docs/skills/overview.md` + `en/` 对应页
  - `docs/agents/limits.md`(envelope 契约,见下节)
  - 执行时以 `grep -rln "write_backend\|desktop_write\|bridge\|--write-backend" docs --include=*.md`
    为准复核,排除 `docs/.vitepress/dist/`。
- 文档构建验证:`npm --prefix docs run build`(script 已存在:`vitepress build .`)必须通过,
  以捕获死链与坏引用。
- `.trellis/spec/zot-cli/backend/desktop-bridge.md` 删除,`index.md` 去引用;
  `merge-dedupe.md` 去掉双后端叙述;`zot-core/backend` config spec 同步。

## 风险与回滚

- 风险:已配对老用户升级后本机 merge 静默变为需要 Web 凭据 —— 由 doctor 迁移提示
  + CHANGELOG 覆盖;不做运行时兜底。
- Zotero 端残留:插件仍装在用户 Zotero 里也无害(只是多一个挂在 23119 的路由),
  卸载指引进 CHANGELOG 与 doctor hint,不做远程 revoke(避免为删除功能再调它一次)。
- 回滚:整任务单分支实现,revert 分支即可;无数据迁移、无 config 写入。
