# Zotero 本机桥接基础

## Goal

建立一个不依赖 Zotero Web API key 的安全桌面写入基础层：最小 Zotero 9 插件、
版本化白名单 loopback 协议、独立 Rust desktop client，以及可操作的 XPI 生成、
配对、状态、撤销和 doctor 诊断。本子任务不接入具体 library mutation。

## Dependencies

- 第一阶段内部无前置子任务。
- 父任务：`07-11-local-zotero-write`。
- 为 `07-11-local-merge-dedupe` 提供稳定 protocol/client/config API；该后续任务在
  本任务验收前不得开始。

## Requirements

- **R1 plugin packaging**：插件源位于 `plugins/zot-bridge/`，首版声明 Zotero 9.x
  兼容；`zot bridge setup` 从 binary 内置资产生成版本化 XPI 并打开目录。
- **R2 whitelist protocol**：只注册 `/zot-bridge/v1/*` 固定 JSON endpoint；没有
  eval、execute、script、通用 field update 或动态 operation。
- **R3 transport security**：POST-only、JSON、loopback Host allowlist、拒绝 Origin、
  bearer auth、body/timeout/replay 限制，shutdown/uninstall 注销 endpoint。
- **R4 pairing**：插件显示五分钟有效、单次使用 code；`bridge pair` 换取长期随机
  token。pair 轮换旧 token，`bridge revoke` 和插件 Reset 可撤销。
- **R5 secret storage**：CLI 在当前 root/profile 保存原 token，插件只保存 token
  hash；配置/Debug/log/envelope 全部脱敏。
- **R6 Rust boundary**：新增 `src/zot-desktop` crate，拥有 DTO、client、handshake、
  timeout 和 error mapping；不得把 client 塞入 `zot-local` 或 `zot-remote`。
- **R7 backend config**：新增 serde-default-web 的 `write_backend` 和 bridge config；
  global `--write-backend` 支持调用级覆盖。pair 成功后当前配置目标设为 desktop。
- **R8 CLI surface**：新增 `zot bridge setup|pair|status|revoke`。status 区分未安装、
  未运行、未配对、auth 失败、协议不兼容和可用。
- **R9 doctor**：JSON 独立输出 local SQLite read、Local HTTP read、desktop bridge
  write、Web API write；每项含 available/configured/error/hint，不互相替代。
- **R10 version guard**：workspace、CLI、XPI manifest、protocol compatibility 有自动检查。

## Acceptance Criteria

- [x] **AC1** `cargo test -p zot-desktop` fake server 覆盖 health/pair/status/revoke、
  401、timeout、invalid JSON、oversize、replay 和 protocol mismatch。
- [x] **AC2** 旧 config 无新字段时反序列化为 web；pair/revoke 按 explicit profile >
  default profile > root 修改唯一目标，命令覆盖不落盘。
- [x] **AC3** token、pair code 不出现在 `Debug`、`config show`、doctor、错误和测试快照。
- [x] **AC4** 无 Web 凭据时 `bridge status` 能识别 Zotero 9.0.6 和 plugin protocol。
- [x] **AC5** 无 token、错误 token、browser Origin、非法 Host、未知字段和非 POST
  请求均不能访问受保护 endpoint。
- [x] **AC6** XPI 在隔离 Zotero 9 profile 可安装、重启、显示 code、pair、revoke、
  shutdown/uninstall 注销；未直接修改 profile 数据库。
- [x] **AC7** `doctor --json` 四种能力字段独立且有稳定 hint。
- [x] **AC8** `just xpi-check`、workspace version guard 和 `just ci` 通过。

## Out of Scope

- merge、note、tag、collection 等具体 mutation endpoint。
- 自动安装 XPI、直接修改 Zotero profile、SQLite 或扩展数据库。
- 任意 JavaScript 执行、browser-callable CORS API、LAN 监听。
