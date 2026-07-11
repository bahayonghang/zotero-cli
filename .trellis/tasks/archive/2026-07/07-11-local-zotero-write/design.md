# Design: Zotero 本机安全写入路线图

## Architecture

```text
zot-cli
  |- local read / duplicate planner -> zot-local (SQLite read-only)
  |- selected writer
  |    |- web     -> zot-remote -> api.zotero.org
  |    `- desktop -> zot-desktop -> 127.0.0.1:23119/zot-bridge/v1/*
  `- standard CommandOutput / CliEnvelope

Zotero 9 plugin
  -> validate loopback request, auth, protocol, scope and item versions
  -> set validated metadata on the in-memory keeper
  -> native mergeItems() transaction
```

新增 workspace crate `src/zot-desktop`。它拥有 bridge DTO、HTTP client、协议版本
协商、timeout、secret-safe Debug 和 bridge error 映射。`zot-local` 继续只读，
`zot-remote` 继续只表示云端写入。

插件源放在 `plugins/zot-bridge/`，使用 Zotero 9 bootstrap extension 结构：
`manifest.json`、`bootstrap.js` 和少量模块。首版兼容声明只覆盖 Zotero 9.x；
handshake 同时返回 plugin、protocol 和 Zotero version，超出验证范围时拒绝 mutation。

## Configuration And Backend Selection

- 新增 `WriteBackend { Web, Desktop }`，serde 缺省值为 `Web`，保证旧配置不漂移。
- root `ZoteroConfig` 与 `ProfileConfig` 都增加 `write_backend` 和
  `desktop_bridge`。materialize profile 时与现有 API key 一起复制到 effective config。
- `DesktopBridgeConfig` 保存长期 token、paired plugin/protocol version 和时间；
  自定义 `Debug` 永远只输出 `(set)`。`config show` 只显示配置状态和版本，不显示 token。
- `zot bridge pair` 修改当前 effective 配置目标：显式 `--profile` 优先，否则使用
  raw config 的 default named profile；两者都不存在时才写 root。成功后把同一目标的
  `write_backend` 设为 desktop。
- `Cli` 增加 global `--write-backend desktop|web`。解析优先级为命令行覆盖 >
  effective profile/root 配置 > serde default web。覆盖不落盘。
- `AppContext` 提供 backend resolver；只在命令真正需要 writer 时构造对应 client。
  dry-run planner 不应仅因默认 backend 是 web 就提前要求 Web 凭据。

## First-Stage Ownership

- bridge foundation 拥有 crate、plugin、config、CLI bridge commands、doctor 和打包。
- merge/dedupe 子任务拥有 writer 抽象、desktop merge DTO 及三个命令入口。
- skill/docs 子任务只在前两项命令和 envelope 稳定后修改 canonical skill 和文档。
- 父任务不直接改生产代码；最后执行跨子任务集成检查。

## Bridge Protocol V1

固定 base URL 为 `http://127.0.0.1:23119/zot-bridge/v1`。插件复用 Zotero 自带
loopback server，不开独立端口、不扫描端口；测试通过 client constructor 注入 fake URL。

白名单端点：

| Endpoint | Auth | Purpose |
| --- | --- | --- |
| `POST /health` | no | plugin/protocol/Zotero version，不返回 library 数据 |
| `POST /pair` | one-time code | 换取长期 token，成功即使 code 失效 |
| `POST /status` | bearer | token、capabilities、可写 library 状态 |
| `POST /auth/revoke` | bearer | 撤销当前长期 token |
| `POST /merge/preview` | bearer | 重算 metadata/结构计划并签发短期 plan token |
| `POST /merge/apply` | bearer | 校验 plan 后执行一个事务 |

所有请求含 `protocol_version`、`request_id`、`sent_at`。bridge response 使用独立但
同形的 `{ok,data,error,meta}` envelope，meta 含 request/plugin/protocol/Zotero version；
`zot-desktop` 将其映射为 `ZotError::DesktopBridge` 和现有 CLI envelope。

## Pairing And Secret Lifecycle

1. `zot bridge setup` 从编译进 binary 的固定插件资产生成版本化 XPI 到
   `~/.config/zot/plugins/`，校验 archive 内容后用 `opener` 打开目录；不修改 Zotero profile。
2. 用户手动安装、重启。插件首次启动和 Tools 菜单动作显示 8 位无歧义字符配对码，
   code 由 CSPRNG 生成，只保存在内存，五分钟过期且单次使用。
3. `zot bridge pair <code>` 调 `/pair`，插件生成 256-bit base64url token。CLI 保存
   原 token；插件只持久化 SHA-256 hash。pair 成功会轮换旧 token。
4. `zot bridge revoke` 先调用插件撤销，再清本地配置。token 丢失时，插件 Tools
   菜单的 Reset Authorization 可本地清除 token 并生成新 code。

配置文件与现有 API key 使用同一用户级信任边界；Unix 写后校验 0600，Windows
依赖用户 profile 目录 ACL。所有日志、fixtures、Debug 和 envelope 使用假 token 或脱敏状态。

## Request Security

- 仅 POST + `application/json`，mutation body 上限 64 KiB；pair/health 上限 4 KiB。
- `permitBookmarklet = false`；拒绝任何 `Origin`，拒绝非 loopback Host；不启用 CORS。
- DTO 拒绝未知字段，operation 是闭合 enum，不存在 script/eval/execute 字段。
- protected request 的 `sent_at` 允许时钟偏差 60 秒；`request_id` + payload hash 在
  10 分钟 replay cache 中唯一。相同 id/相同 payload 返回缓存结果，不同 payload 拒绝。
- merge apply 使用额外 `operation_id` 和短期 plan token。响应丢失后重试返回原结果
  或 `already_applied: true`，不得重复合并。
- 日志只记录 operation、request id、耗时和结果码；不记录 Authorization、code、
  token、标题、note 内容或完整 payload。
- health/status timeout 5 秒，merge preview/apply 120 秒；网络 timeout 不触发另一个后端。

## Scope And Merge Contract

CLI 协议只发送 `{type:"user"}` 或 `{type:"group",group_id:<public-id>}`。插件用
`Zotero.Libraries.userLibraryID` 或 group public ID 查找 local library ID，禁止 CLI
直接提供 internal ID。插件验证 library editable、keeper/source 顶层类型、同库、
未删除和当前版本/指纹。

desktop preview 保留 keeper item type，按 Zotero field/base-field 映射只填 keeper
合法且为空的 metadata；不覆盖非空值。apply 在事务内重算并比对 preview：先写
metadata，再调用原生 `mergeItems(keeper, sources)`。原生逻辑拥有 children、tags、
collections、relations、`dc:replaces`、duplicate attachment 和 trash 语义。

`library dedupe` 默认 dry-run 仍是本地 planner，不要求任何 writer。confirm 时先过滤
confidence，再逐组调用 selected writer；desktop 每组执行 preview+apply，web 继续
现有引擎。组内 desktop 原子，组间允许部分成功并结构化汇总。

## Errors And Compatibility

稳定错误码至少包括：`bridge-unreachable`、`bridge-not-installed`、`bridge-unpaired`、
`bridge-auth`、`bridge-protocol`、`bridge-timeout`、`bridge-invalid-request`、
`bridge-library-not-found`、`bridge-library-readonly`、`bridge-item-not-found`、
`bridge-item-changed`、`bridge-cross-library`、`bridge-plan-expired`、`bridge-replay`、
`bridge-zotero-shutdown`、`bridge-transaction`。

旧 config 缺少新字段时选择 web。CLI JSON 只增加 `write_backend`、bridge status 和
desktop 结果字段；Envelope `api_version` 保持 1，除非实现期发现真正的破坏性变化。

## Rollout And Rollback

- 先在 fake server 和隔离 Zotero profile 验证，再使用可恢复 fixture collection。
- XPI 只写插件偏好和 Zotero API 事务；卸载后 CLI 的 desktop 配置会变为 unavailable，
  不自动切 web。用户可显式 `config set write-backend web` 或重新配对。
- 数据回滚依赖 Zotero 回收站和隔离测试备份；第一阶段不提供永久删除。
