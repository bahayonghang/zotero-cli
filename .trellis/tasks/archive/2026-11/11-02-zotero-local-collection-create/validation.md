# 最新结论（覆盖下方历史未授权状态）

2026-11-02 核心实现与验收完成，进入 review 待提交收口。Windows Zotero 10.0.7 独立 profile/data：标签增删、根/子分类创建、3 条不同决策批量 apply、API/UI/正常重启回读通过；原实例已恢复。173 个 CLI 测试及 workspace/质量门通过。故障注入与真实桌面证据分列；用户要求聚焦核心，不扩展桌面故障测试。未 commit/push/PR，未归档。见父 validation.md。

真实证据：父任务 acceptance/run.py 与 ignored target/local-organization-acceptance/2026-11-02 下的场景 JSON、plan.json、report.json、restart.json；父 validation.md 汇总 AC 与边界。

# Validation — 本地分类创建

日期：2026-11-02。分支 `dev`。未 commit、未 push。调用路径仍是 `cargo run -q -p zot-cli -- --json`。
没有启动 Zotero，没有对真实库创建分类、授权或重启。

## 协议核查

实现前读取官方文档，而不是当前不可达的桌面实例：

- https://www.zotero.org/support/dev/web_api/v3/basics
- https://www.zotero.org/support/dev/web_api/v3/write_requests
- https://www.zotero.org/support/dev/web_api/v3/local_api

与 design.md 一致，因此没有改成无条件写入：

- 多对象响应带 `Last-Modified-Version`，本地版本独立于 Web。
- Local API 省略 limit 时返回完整列表；`limit`/`start` 和 Link 仍可用。
- collection POST 使用库版本 `If-Unmodified-Since-Version` 或 Write-Token。版本化请求不应再混用 Write-Token。
- HTTP 200 只表示请求完成，对象结果在 `successful` / `unchanged` / `failed`。

缺库版本、分页版本变化或列表无法收口时，实现返回协议/冲突错误，不 POST。

本轮 doctor 仍是 Local API 不可达、Web 凭据缺失、SQLite 可读。这不是桌面验收。

## 自动化检查

| 检查 | 结果 |
| --- | --- |
| `cargo test -p zot-desktop --locked` | PASS，22 个测试 |
| `cargo test -p zot-cli --locked` | PASS，包含 local_create 与 json_error_contract；`just ci` 的 workspace 测试一并通过 |
| `just ci` | PASS |
| `npm --prefix docs run build` | PASS |
| `git diff --check` | PASS；仅 LF/CRLF 提示 |

## AC 映射

| AC | mock | 真实 CLI | 真实桌面 | 结论 |
| --- | --- | --- | --- | --- |
| C-AC1 无 Web 凭据可预览并创建根/子分类，API/UI 名称、父级、key 一致 | PASS 预览、POST、回读 | 未覆盖成功创建 | 未授权，未做 | 部分完成 |
| C-AC2 同级同名拒绝、不同父级允许；版本变化在提交前拦截 | PASS | 未覆盖 | 未授权，未做 | 部分完成 |
| C-AC3 库版本不与 item/Web version 混用；根分类也有条件保护 | PASS | 参数契约进程测试 | 未授权，未做 | 部分完成 |
| C-AC4 对象失败、错误身份、超时/5xx、畸形响应、回读失败分别报告，无盲重试 | PASS | 未覆盖 | 未授权，未做 | 部分完成 |
| C-AC5 所得 key 可交给现有 add-item；创建不改成员/children/附件 | 文档示例 + POST body 不含成员 | 未串联真实 add-item | 未授权，未做 | 部分完成 |
| C-AC6 Web/connector 回归、文档/skill、隔离库 UI 验收 | 回归与文档 PASS | 错误契约进程测试 | 未授权，未做 | 不能标完成 |

### 证据要点

- mock：空库版本 0 被接受为真实 header，不是缺失版本。缺 header、Total-Results 不闭合、分页版本变化都在 POST 前失败。分页只提取 `start`，不请求 Link 里的外部 URL。
- mock：根分类 POST body 是 `[{"name","parentCollection":false}]`，子分类带父 key。header 含实例 ID、API key 和 `If-Unmodified-Since-Version: 12`，没有 Write-Token。
- mock：200 `failed` 是错误且不重试。500/204/错误实例是 `unknown`，一次请求。unknown 不按名称再查分类。401 只重新授权一次。回读名称不符是 `verification-failed`，不是 applied。
- mock：同级精确同名，包括已删除对象，拒绝并列出 key；不同父级允许同名。父分类缺失、已删除或路径成环在授权前拒绝。
- 真实 CLI：Web 确认参数返回 `local-write-options`；空白名称返回 `local-write-collection`；非法 parent 返回 `local-api-key`。stdout 是单个 envelope，stderr 为空。
- 创建命令没有调用 add-item。文档把返回 key 写成现有本地 add-item 的输入示例。
- 真实桌面的根/子分类、UI、重启和授权框未做。不能把 mock 写成桌面证据。

## Codex 接续更新（2026-11-02）

上述为 Grok 首轮记录。接续已通过最终 just ci、文档构建、diff 检查；B 另补矛盾/畸形创建回执回归并修复。用户已授权隔离桌面验收，独立 profile/data 和三条合成文献已建立，当前等待用户手动启用测试实例 Local API；尚未通过真实写入/UI/重启验收。最新环境、证据与恢复要求以父任务 validation.md 为准。
