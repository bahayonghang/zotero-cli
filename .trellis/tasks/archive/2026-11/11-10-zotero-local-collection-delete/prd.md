# 本地删除一个 Zotero 分类

## Goal

用户明确指定一把分类 key 后，可以在不配置 Web API 的情况下，通过 Zotero 10+ Local API 预览并确认删除该分类。Agent 从 skill 就能知道删除会带走子分类、文献仍留在库里，以及确认必须绑定哪三个值。

## Background

`.trellis/tasks/11-09-zotero-collection-taxonomy` 继续保持 `planning`，本任务不是它的子任务，也不改它的文档或库操作记录。

本机 Zotero 10.0.8 的单对象 Local API 已支持 `DELETE /collections/:key`。`zot collection delete` 目前无论是否传入本地参数，都只调用 `ctx.remote()`。没有 Web 凭据时这条命令不能用。证据见 `research/erase-semantics.md`。

## Requirements

- D1：`zot collection delete KEY` 不带 `--write-backend local` 时仍立即走 Web API，参数和结果保持现状。Web 路径拒绝 `--confirm`、`--expected-server-id`、`--expected-version`、`--expected-descendants-sha256`。
- D2：显式 `--write-backend local` 时默认只预览。预览不读取授权文件、不请求授权、不发送 DELETE，也不打开 `zotero.sqlite`。
- D3：预览列出 Local API 分类列表返回的目标分类和全部子孙分类，并给出这些 key 的 SHA-256。响应里若带 `deleted`，该标记进入列表和哈希。Zotero 10.0.8 的 `GET /collections` 不返回废纸篓分类，也不识别 `includeTrashed`，所以这些 key 不会出现在预览里；确认后的 DELETE 仍会把未列出的废纸篓子分类一起永久擦除。文案必须说明这一点。文献保留在库里，只是不再属于这些分类。子分类被销毁，不会提升到父级。不得为了补全废纸篓而打开 SQLite。
- D4：确认必须同时带 `--confirm --expected-server-id --expected-version --expected-descendants-sha256`。版本是预览中的分类对象版本，不是库版本，也不是条目版本。预览返回 0 时，0 是合法确认值。
- D5：确认在授权前重新计算 Server ID、对象版本和子孙哈希。任一不符就拒绝，不授权、不删除。
- D6：只有 DELETE 返回 204 且 Server ID 一致，随后 GET 得到 404，才是 `applied` 且 `verified`。HTTP 200、超时、5xx 或 Server ID 不符是 `unknown`。204 之后对象仍在是 `verification-failed`。DELETE 本身 404 是 `absent`，不得声称这次调用删除了它。`unknown` 之后的 404 不能改记为这次删除成功。这些非成功状态不得盲重试。
- D7：只删除这一把 key 对应的单对象。不调用 50-key 批量 DELETE，不自动改走 Web、connector 或 SQLite。
- D8：`skills/zot` 与对应中英文档不再把分类删除说成只有 Web 路径。重命名和重挂父级仍只走 Web。

## Acceptance Criteria

- [x] AC1：不带 local 的 `collection delete KEY` 仍只调用 Web；带上本地确认参数时 Web 路径拒绝。
- [x] AC2：本地预览不授权。确认的 Server ID、对象版本或子孙哈希不符时不发送 DELETE。
- [x] AC3：对象版本 0 可以确认；空子孙集合的哈希是空字节串的 SHA-256。
- [x] AC4：204 加一致 Server ID 再 GET 404 为 applied/verified。204 后 GET 200 为 verification-failed。200、5xx、错误 Server ID 为 unknown，且不会再被一次 GET 404 改成成功。DELETE 404 为 absent。412 是冲突，不是 unknown。
- [x] AC5：预览包含列表里返回的子孙分类；子孙集合变化会改变哈希。文献删除不在本命令的请求里。废纸篓子分类不在 10.0.8 的列表里，文档说明 DELETE 仍会擦除它们。
- [x] AC6：`skills/zot`、镜像、CLI 文档和 spec 中“删除没有本地路径”的句子已改掉；重命名和重挂父级仍是 Web-only。
- [x] AC7：自动化测试使用 mock Local API。不对本机真实库执行删除。

## Out of scope

分类重命名、重挂父级、批量删除、按名称查找后删除、删除文献、把用户库里已搬空的旧分类真正删掉、git commit、GitHub issue，以及改动 `11-09-zotero-collection-taxonomy`。

## Dependencies

沿用现有 Local API 客户端、授权缓存和 `collection create` 的预览/确认分层。不新增传输，也不改变 Web `delete_collection`。
