# Design — connector-based local write path

## 边界与落点

- **crate 落点**:connector client 放进现有 `src/zot-desktop`(已有 reqwest client、
  loopback 校验 `parse_loopback_url`、`LocalHttpStatus`、tiny_http 测试基建)。
  新建 `src/zot-desktop/src/connector.rs`,不触碰 bridge 代码(删除属于下一个子任务)。
- **最终形态预告**:remove-bridge-plugin 完成后,zot-desktop = "Zotero 内置本机 HTTP 面
  client"(`/api/` 探测 + `/connector/` 导入),本任务的模块切分要为此服务,
  connector 代码不得 import bridge model。

## Connector client 契约

```text
GET  /connector/ping                    -> ConnectorPing { available, zotero_version }
POST /connector/getSelectedCollection   -> SelectedTarget { id, name, editable, library_editable? }
POST /connector/import?session=<uuid>   -> ImportResult { status, session, response(JSON 数组或原文) }
```

- 请求头:`X-Zotero-Connector-API-Version: 3`(对齐 ref 实现 `CONNECTOR_HEADERS`)。
- `import` body 为 BibTeX/RIS 原文,`Content-Type: text/plain`;session id 用
  `zot-<uuid>` 前缀便于在 Zotero 日志中辨识。
- 超时:连接 5s / 请求 30s(导入大 bib 比 bridge 探活慢,不复用 10s)。
- host 仅允许 loopback;base URL 支持 `ZOT_CONNECTOR_BASE_URL`(与 ref 的
  `ZOTERO_LOCAL_BASE_URL` 同理)覆盖,默认 `http://127.0.0.1:23119`。

## 错误模型

- `zot-core` 新增 `ZotError::Connector { code, message, hint, status }`
  (不复用 `DesktopBridge` 变体——它将随下个子任务删除)。
- 错误码:`connector-unreachable`(连接失败,hint「启动 Zotero 后重试」)、
  `connector-timeout`、`connector-http`(非 2xx)、`connector-import-format`
  (格式判定失败)、`connector-target-readonly`(选中目标不可写)。禁止任何 fallback 到 Web API。

## CLI 命令面

- 命令:`zot item import (--file <path> | --text <string>) [--format bibtex|ris] [--confirm]`
  - 确认 flag 采用项目既有 `--confirm` 约定(merge/dedupe 同款),不引入 ref 的 `--yes`。
  - 不带 `--confirm` = dry-run:调 `getSelectedCollection` + 本地解析记录数,输出
    `{ target, editable, entries, format, confirmed: false }`,不发 import 请求。
  - 带 `--confirm`:先 `getSelectedCollection` 取目标;**若 `editable` 或
    `library_editable` 为 false,直接报 `connector-target-readonly`,不发 import**;
    可写才发 `import`。
- 格式判定:`--format` 优先;否则扩展名(`.bib`/`.ris`);否则内容嗅探
  (`@<type>{` → bibtex,`TY  - ` → ris);判不出报 `connector-import-format`。
- 记录数统计:bibtex 用 `@\w+\s*\{` 计数(ref 同款正则);ris 用 `^TY  - ` 计数。
- 不新增独立 `selected-target` 子命令:dry-run 输出已覆盖该需求,doctor 覆盖探活。

## doctor 变更

- `capabilities.connector_write = { configured: true, available: <ping ok>, hint? }`
  - available=false 时 hint:「Start Zotero to enable local import」。
  - 文案与 JSON 中注明 scope:`"scope": "import-only"`,防止 agent 误判成通用写后端。
- 本任务不动 `desktop_write` / `local_http_read`(重排在 remove-bridge-plugin 内完成)。

## 数据流

```text
item import --confirm
  └─ ConnectorClient.ping()            # 失败→ connector-unreachable,停
  └─ ConnectorClient.selected_target() # 取目标 collection + editable/library_editable
  └─ editable 校验                      # 任一 false → connector-target-readonly,停(不发 import)
  └─ 本地读文件/text → 判格式 → 数记录
  └─ ConnectorClient.import(session, text)
  └─ envelope: { session, target, editable, entries, format, status }
```

## 兼容与回滚

- 纯新增,无既有行为变更;回滚 = revert 本任务提交。
- Zotero 侧兼容:connector server 自 Zotero 5 起存在,`/connector/import` 走内置
  translator,Zotero 7/8/9 均可用;不做版本门控,失败时用 HTTP 状态照实报错。

## Tradeoffs

- 导入目标由 Zotero UI 当前选中状态决定,CLI 不能指定 collection。替代方案
  (先 saveItems 建 session 再 updateSession 指定 target)复杂度高且依赖非稳定
  payload 结构,MVP 不做;dry-run 复述目标名即为风险缓解。
