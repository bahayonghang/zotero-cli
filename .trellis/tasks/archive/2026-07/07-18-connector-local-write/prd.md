# Add connector-based local write path (import via built-in connector server)

## Goal

在 `zot` CLI 中新增基于 Zotero **内置 connector server**(`http://127.0.0.1:23119/connector/*`)
的本机写入路径:把 BibTeX / RIS 记录导入到正在运行的 Zotero(当前选中的库/collection),
无需 Web API key、无需任何插件。对齐 `ref/zotero/skills/zotero/scripts/zotero.py` 的
`import-bibtex` / `import-ris` / `selected-target` 能力。

## Requirements

### R1 connector client(zot-desktop crate 内新增)

- `GET /connector/ping`:探测 connector 可用性,取 `X-Zotero-Version` 头。
- `POST /connector/getSelectedCollection`:返回当前选中库/collection(id、name、可编辑性)。
- `POST /connector/import?session=<uuid>`:body 为 BibTeX/RIS 原文(`text/plain`),
  导入到当前选中目标;请求头带 `X-Zotero-Connector-API-Version: 3`。
- 仅允许 loopback host(复用现有 `parse_loopback_url` 约束)。

### R2 CLI 命令面

- `zot item import --file <path> | --text <string>`(或等价命名,design.md 定):
  格式自动按内容/扩展名判定为 BibTeX 或 RIS;`--format bibtex|ris` 可显式覆盖。
- `zot item import` 属于写操作,采用项目**既有 `--confirm` 约定**(与 merge/dedupe 同款),
  **不引入 ref 的 `--yes`**:不带 `--confirm` 时先输出目标(selected collection 名称)与
  记录数,拒绝静默写入。全任务文档统一用 `--confirm`,不得再出现 `--yes`。
- 新命令支持 `--json` envelope,输出导入 session id、目标 collection、HTTP 状态。
- 暴露"查看导入目标"的只读入口(dry-run 输出已覆盖,doctor 覆盖探活;不新增独立子命令)。

### R3 目标可写性强制检查(不可省)

- `getSelectedCollection` 返回的 `editable` / `library_editable` 任一为 `false` 时,
  **在发送 `/connector/import` 之前**拒绝,报结构化错误(如 `connector-target-readonly`),
  hint 提示"在 Zotero 中选中一个可写的 collection"。只读 group / feed 是真实场景。
- dry-run 输出中也要展示目标可写性,让用户 confirm 前就能看到。

### R4 doctor 能力位

- 新增 `capabilities.connector_write`:Zotero 运行且 `/connector/ping` 通过即 available;
  不依赖任何配置。文案标注 `scope: import-only`,提示"仅支持导入新条目,不能修改已有条目"。
- 保留现有 `capabilities.local_http_read`(`/api/` 探测)不变。

### 边界(明确不做)

- 不做 `/connector/saveItems` 结构化条目保存(translator payload 复杂,由 Web API
  `item create` 已覆盖;后续需要再立任务)。
- 不做 local API pref(`extensions.zotero.httpServer.localAPI.enabled`)的写入/翻转与
  Zotero 进程重启(ref 里的 `enable --restart`):zot 的读路径是直连 SQLite,不依赖
  local API;doctor 对 local_http_read 不可用时给一句启用提示即可。
- 不改动 bridge 代码(删除在子任务 remove-bridge-plugin 中进行)。

## Acceptance Criteria

- [ ] Zotero 运行、未装任何插件时:`zot --json item import --file refs.bib --confirm` 成功,条目出现在 Zotero 当前选中 collection
- [ ] RIS 文件同样可导入;格式覆盖 flag 生效
- [ ] Zotero 未运行时报结构化错误(code 指向 connector 不可达,hint 提示启动 Zotero),不 fallback 到 Web API
- [ ] 选中目标为只读 group/feed(`editable` 或 `library_editable` 为 false)时,`import` 在发请求前被拒,报 `connector-target-readonly`;fake-server 测试覆盖此路径
- [ ] 不带 `--confirm` 时不产生写入(fake-server 断言未收到 `/connector/import`),输出目标、可写性与记录数
- [ ] `zot --json doctor` 出现 `capabilities.connector_write`(含 `scope: import-only`)且状态正确(Zotero 开/关两态)
- [ ] 单元测试覆盖 connector client(fake HTTP server,复用 client.rs 现有 tiny_http 测试模式)
- [ ] 最终门:`just ci` 全绿(fmt / check / clippy / test / skills-check)

## Notes

- 参考:`ref/zotero/skills/zotero/references/local-api-routes.md`(connector 路由),
  `ref/zotero/skills/zotero/scripts/zotero.py` `cmd_import_records` / `cmd_selected_target`。
- connector 写入的目标由 Zotero UI 当前选中状态决定,CLI 无法指定 collection——文档与
  输出中必须讲清这一点,避免用户误以为可以定向导入。
