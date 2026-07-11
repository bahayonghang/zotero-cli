# Design: Zotero 本机桥接基础

## Files And Boundaries

```text
plugins/zot-bridge/
  manifest.json
  bootstrap.js
  modules/protocol.js
  modules/auth.js
  modules/ui.js

src/zot-desktop/
  Cargo.toml
  src/lib.rs
  src/client.rs
  src/model.rs
  src/error.rs

src/zot-cli/
  cli.rs / cli/args.rs
  commands/bridge.rs
  commands/doctor.rs
  context.rs
```

插件保持无 Node runtime、无 bundler 的小型 bootstrap extension。模块数量以可测试性
为限，不复制 Zotero 内部库代码。`zot-desktop` 依赖 `zot-core` 的 `LibraryScope` 和
error/envelope primitives；`zot-cli` 负责编排 config 与用户输出。

## XPI Generation

插件资产作为固定文件通过 `include_bytes!/include_str!` 嵌入 `zot-cli`。setup 使用
`zip` writer 在运行时生成确定性 XPI 到 `~/.config/zot/plugins/zot-bridge-<version>.xpi`，
写临时文件后原子 rename，并打开目录。archive entry 列表是代码中的白名单，不能把
仓库任意文件打包进去。

`just xpi-check` 生成临时 XPI，检查必需 entry、manifest JSON、插件 ID、workspace
version 和禁止文件；不提交生成的 XPI。

## Zotero 9 Extension Lifecycle

- plugin ID 固定为 `zot-bridge@bahayonghang`。
- manifest 的 `strict_min_version` 为已验证的 Zotero 9 基线，`strict_max_version`
  限制在 `9.*`；扩大兼容范围必须经过真实 smoke。
- `startup` 注册 endpoint 和 Tools 菜单；`shutdown`/`uninstall` 删除 endpoint、菜单、
  pairing code 和 replay cache。
- 首次启动弹出 pairing dialog；Tools 菜单保留 Show Pairing Code 和 Reset Authorization。

## Protocol V1

base URL：`http://127.0.0.1:23119/zot-bridge/v1`。首个子任务只实现：

```text
POST /health
POST /pair
POST /status
POST /auth/revoke
```

基础 request：

```json
{
  "protocol_version": 1,
  "request_id": "uuid-v4",
  "sent_at": "2026-07-11T00:00:00Z",
  "client": { "name": "zot", "version": "0.6.0" }
}
```

基础 response：

```json
{
  "ok": true,
  "data": {},
  "meta": {
    "request_id": "uuid-v4",
    "protocol_version": 1,
    "plugin_version": "0.6.0",
    "zotero_version": "9.0.6"
  }
}
```

error 包含稳定 `code/message/hint/retryable`。client 要求 `protocol_version == 1` 且
plugin capability 声明包含调用操作；不做模糊降级。

## Authentication

- pairing code 使用 CSPRNG 生成 8 位无歧义 base32，内存保存 hash、expiry、used；
  五分钟后或成功调用后清除。
- `/pair` body 含 code 和 client instance ID。成功生成 32-byte token，返回一次。
- 插件 preference 只存 SHA-256 token hash 和授权时间；CLI config 存原 token。
- bearer 使用常量时间 hash 比较。pair 成功覆盖旧 hash；revoke 删除 hash。
- plugin Reset 不依赖旧 token，用 Zotero UI 本地动作撤销并生成新 code。

## HTTP Guards

- endpoint 仅 `supportedMethods = ["POST"]`，`supportedDataTypes = ["application/json"]`，
  `permitBookmarklet = false`。
- Host 只允许 `127.0.0.1:23119`、`localhost:23119`、`[::1]:23119`；出现 Origin 即拒绝。
- health/pair body 最大 4 KiB，status/revoke 最大 16 KiB；unknown field 拒绝。
- protected request 校验 bearer、±60 秒 sent_at、request_id。10 分钟 LRU replay cache
  对相同 id+hash 返回缓存，对 id 不同 payload 冲突返回 `bridge-replay`。
- client health/status timeout 5 秒，pair/revoke 10 秒；URL 只允许 loopback HTTP，
  production constructor 不接受远程 host，test constructor 可注入 fake listener。

## Configuration Shape

```toml
[zotero]
write_backend = "desktop"

[zotero.desktop_bridge]
token = "..."
plugin_version = "0.6.0"
protocol_version = 1
paired_at = "..."
```

named profile 使用相同字段。pair/revoke 的目标解析为 explicit profile > default profile >
root，必须与 `AppConfig::load` 的 effective materialization 一致。`WriteBackend` 自定义
serde lower-case；缺省 web。
`DesktopBridgeConfig` 自定义 Debug。`AppConfig::save` 写完后在 Unix 设置 0600；Windows
不调用外部 ACL 命令，沿用用户 profile 目录权限并在 docs 说明信任边界。

`config set write-backend desktop|web` 可手动选择；不提供 `config set bridge-token`，
token 只能由 pair 写入，减少 shell history 泄漏。

## CLI Commands

- `bridge setup [--output <dir>]`：生成/校验 XPI并打开目录，返回路径、plugin/version、
  next steps；不假装已安装。
- `bridge pair <code>`：先 health，再 pair；成功原子更新当前 config target 和 backend。
- `bridge status`：health 后在有 token 时调用 protected status；输出 installation、pairing、
  protocol、Zotero version 和 capabilities。
- `bridge revoke`：调用 revoke 后清本地 token；若 Zotero 不可达则默认不静默清理，
  返回 hint 指向 plugin Reset，可由显式 `--local-only` 清本地残留。

## Doctor Contract

```jsonc
{
  "capabilities": {
    "local_sqlite_read": {"available": true},
    "local_http_read": {"available": true, "version": "9.0.6"},
    "desktop_write": {"configured": true, "available": true, "protocol_version": 1},
    "web_write": {"configured": false, "available": false}
  }
}
```

旧 `write_credentials` 字段先保留并标为 web-specific，避免消费者突变；新代码只增加
capabilities。desktop unavailable 不改变 selected backend。

## Error Mapping

`zot-desktop` 使用 `ZotError::DesktopBridge`，保留 HTTP status 供内部诊断，但 CLI error
payload 只输出稳定 code/message/hint。reqwest connection refused 根据 `/health` 语境映射
`bridge-unreachable`，不能武断区分“Zotero 未运行”和“插件未安装”；若 Local HTTP probe
成功而 bridge health 404，才映射 `bridge-not-installed`。

## Test Strategy

- Rust DTO serialization、secret Debug、config migration、backend precedence 单测。
- tiny fake server 验证 header、timeouts、status、error mapping、replay response。
- JS 协议纯函数用 Zotero test harness 或最小 mock 测 schema/auth/replay。
- XPI archive/manifest/version 测试。
- 隔离 Zotero 9 profile 手工 smoke，记录命令、预期和恢复步骤，不保存真实 token。
