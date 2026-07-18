# Implement — connector-based local write path

前置:阅读 `.trellis/spec/zot-cli/backend/index.md`、`zot-core/backend/index.md`
及 error/output 相关 spec;参考 `ref/zotero/skills/zotero/scripts/zotero.py`
的 `cmd_import_records` / `cmd_selected_target` / `CONNECTOR_HEADERS`。

## Checklist

1. [ ] `zot-core`:新增 `ZotError::Connector { code, message, hint, status }` 与
       错误码常量;错误展示格式对齐既有 `DesktopBridge` 变体风格。
       → verify: `cargo test -p zot-core`
2. [ ] `zot-desktop/src/connector.rs`:`ConnectorClient`(ping / selected_target /
       import),model 类型 `ConnectorPing` / `SelectedTarget { editable, library_editable, .. }`
       / `ConnectorImportResult`;loopback 校验、超时、header 按 design.md。
       → verify: tiny_http fake server 单测(成功、非 2xx、超时、非 loopback 拒绝、
       **只读目标 editable=false**)
3. [ ] `zot-desktop/src/lib.rs`:导出新类型;不改 bridge 导出。
       → verify: `cargo build -p zot-desktop`
4. [ ] `zot-cli` args:`ItemCommand::Import(ItemImportArgs { file, text, format, confirm })`,
       file/text 互斥必选一;`--format` 枚举 bibtex|ris。
       → verify: `cargo run -q -p zot-cli -- item import --help`
5. [ ] `zot-cli/src/commands/item/import.rs`:格式判定 + 记录计数 + dry-run/confirm
       两态 + **editable 强制检查**(confirm 分支发 import 前,任一 false → 报
       `connector-target-readonly`)+ envelope 输出;命令注册进 `commands/item/mod.rs` 路由。
       → verify: 单测覆盖格式嗅探、计数、只读目标拒绝;`cargo test -p zot-cli`
6. [ ] doctor:`capabilities.connector_write`(ping 探测、`scope: import-only`、hint),
       文本模式加一行 `Connector write: ...`;更新 doctor 单测。
       → verify: `cargo test -p zot-cli doctor`
7. [ ] 全量门(与本仓库 CI 一致,不用零散 cargo 命令拼)。
       → verify: `just ci`(= version-check / fmt / check / clippy / test / skills-check)
8. [ ] 手工端到端(实机):
   - Zotero 开、选中可写 collection:`zot --json item import --file sample.bib`
     (dry-run 显示目标+可写性)→ `--confirm` 后条目出现在选中 collection;RIS 同样验证一次。
   - 选中只读 group/feed:`--confirm` 报 `connector-target-readonly`,Zotero 无写入。
   - Zotero 关:命令报 `connector-unreachable`;`zot --json doctor` 中
     `connector_write.available=false`。

## Review gate

- dry-run 决不发 `/connector/import` 请求(fake server 断言未收到该 path)。
- 只读目标在 confirm 分支也决不发 import(fake server 断言)。
- 错误路径无 Web fallback;hint 文案不提 bridge。

## Rollback

- 纯新增改动,单 commit 序列,revert 即回滚;无配置/数据迁移。
