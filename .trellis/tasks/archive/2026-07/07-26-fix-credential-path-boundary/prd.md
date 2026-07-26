# P1热修:凭据边界(API key跨域+workspace路径逃逸)

## Goal

关闭两个可独立触发但共同属于信任边界失效的 P1 安全问题：附件上传不得把
Zotero API key 发送给外部授权主机，workspace 名称不得让 TOML 或 RAG sidecar
访问配置根目录之外的路径。

## Background

- 审计证据 `zotero-cli-code-audit-2026-07-25.md:117,120` 已确认两项缺陷。
- `src/zot-remote/src/zotero.rs:66-89` 的通用 request builder 无条件附加
  `zotero-api-key`，而 `upload_attachment()` 会把 API 返回的 `upload_url` 交给它。
- `src/zot-local/src/workspace.rs:40-129` 只有 `create()` 校验名称；`exists()`、
  `path_for()`、`save()`、`load()`、`delete()` 均接受裸字符串。
- `src/zot-local/src/workspace_rag.rs:29-37` 直接用裸 workspace 名构造
  `<name>.idx.sqlite`；CLI 的 workspace 命令均把参数字符串直接传到这些 sink。
- 已归档的 `07-26-fix-pdfium-cwd-rce` 不属于本子任务，不重新实施或归档。

## Requirements

### R1 Zotero 认证按 origin 收敛

- Zotero Web API 请求继续携带 `zotero-api-key`。
- 外部附件上传请求必须使用无 Zotero 凭据的 request builder；不得携带
  `zotero-api-key` 或其他由 Zotero API client 注入的认证 header。
- 生产路径只允许 `https` 外部上传 URL，并在发送请求前以稳定的 typed error
  拒绝其他 scheme。测试构造器可仅为 loopback fake server 开启 HTTP，不能影响
  生产构造器。
- 附件创建、授权、外部上传、注册的既有成功顺序与状态检查保持不变。

### R2 Workspace 名称成为类型边界

- 引入公开的 validated `WorkspaceName` value type；唯一合法格式为
  `^[a-z0-9]+(-[a-z0-9]+)*$`。
- 空字符串、`.`、`..`、路径分隔符、绝对路径、Windows drive/UNC/prefix、大小写
  和非 kebab-case 输入必须在任何文件或数据库访问前返回
  `invalid-workspace-name`。
- `WorkspaceStore` 的 create/exists/load/delete 路径入口和
  `WorkspaceRagStore::open` 必须消费 validated name，而不是裸字符串；`save()`
  必须重新校验 `Workspace.name`，防止反序列化或手工构造绕过类型边界。
- workspace TOML 与 `<name>.idx.sqlite` 的目标必须位于 canonical workspace root
  内；既有 symlink/reparse target 解析到 root 外时必须 fail closed。
- `.md_cache.sqlite` 仍保持 workspace root 共享 sidecar，本任务不改变其布局。

### R3 兼容性与范围

- 合法 kebab-case workspace 的 CLI 行为、序列化名称、TOML/sidecar 文件名保持不变。
- 不增加生产依赖，不直接写 Zotero 主数据库，不处理长期架构项或其他审计条目。

## Acceptance Criteria

- [x] 双 fake-server 回归测试完整执行附件上传，Zotero API server 的请求含 key，
      upload server 的请求不含 key。
- [x] 生产策略测试证明 HTTP 和非 HTTP(S) 外部上传 URL 在网络发送前被拒绝。
- [x] `WorkspaceName` 表格/性质测试覆盖合法 kebab-case，以及 traversal、separator、
      absolute、Windows drive/UNC/prefix 和大小写输入。
- [x] store 的 create/load/delete/save/exists 与 RAG open 不存在裸名称路径 sink；
      非法名称不能在 root 外创建、读取、删除或打开 sidecar。
- [x] 支持的平台上，指向 root 外的 workspace TOML 或 RAG sidecar symlink 被拒绝；
      合法 workspace round-trip 和 RAG 路径布局测试继续通过。
- [x] `cargo test -p zot-remote`、`cargo test -p zot-local`、相关 CLI 测试及最终
      `just ci` 全部通过。

## Out Of Scope

- SecretString/config redaction、通用 HTTP retry/API version、附件 orphan 补偿。
- 通用 filesystem sandbox/no-follow abstraction，或完整 Windows ACL/reparse-point 框架。
- 父任务列出的 application/use-case 层和完整 mutation journal。
