# P2:远程 HTTP 韧性与不可信下载防护

## Goal

关闭远程边界上的五类已确认缺陷：让可安全重放的请求在限流或服务端短暂失败时有界
恢复；固定 Zotero Web API 协议版本；将 OA provider 返回的 PDF URL 当作不可信输入；
在附件多阶段上传失败后补偿孤儿 item 并控制内存；用真正的 XML parser 解析 arXiv
Atom 响应。

## Background

- 审计证据 `zotero-cli-code-audit-2026-07-25.md:137-140,167` 与父任务映射已确认
  本任务覆盖 retry/`Retry-After`、API version、错误体、OA SSRF/资源上限、附件
  orphan/内存和 arXiv regex 六个条目。
- `src/zot-remote/src/http.rs:24-106` 只有 connect/request timeout；所有 remote client
  直接 `.send()`，且非成功响应会把完整、未清洗 body 拼入错误。
- `src/zot-remote/src/zotero.rs:71-95` 集中附加 API key，但未声明
  `Zotero-API-Version`；仅 create item/collection/search 的 POST 带
  `Zotero-Write-Token`，版本条件 PUT/PATCH/DELETE 不带 token。
- `src/zot-cli/src/commands/item/write.rs:298-351` 对 provider URL 使用默认自动重定向和
  `.bytes()`，未校验 scheme、DNS/IP、逐跳 redirect、大小、content type 或 `%PDF-`。
- `src/zot-remote/src/zotero.rs:405-461,689-769` 先创建 attachment item，之后任一失败
  都不清理；授权先读取完整文件，再构建第二个完整上传 `Vec`。
- `src/zot-remote/src/oa.rs:28-48,150-160,301-348` 以 regex 解析 Atom XML；父任务已将
  该 P3 设计债明确映射到本子任务，因此本轮一并关闭。

## Requirements

### R1 有界且资格明确的 HTTP retry

- 共享 HTTP 层最多发送 3 次；只重试幂等 GET，或携带同一
  `Zotero-Write-Token` 的可克隆请求。普通 POST、外部附件上传、以及使用版本前置条件
  的 PUT/PATCH/DELETE 只发送一次。
- 资格请求仅在 transport failure、429 或 5xx 时重试；4xx（除 429）不重试。
- 429/503 等响应的 `Retry-After` 同时支持 delta-seconds 与 HTTP-date；等待时间必须
  有上限。无有效 header 时使用有界指数 backoff 与 jitter，测试可用零等待响应。
- 所有适用 remote client 的 GET 通过共享发送层，Zotero write-token 创建请求也通过
  同一层；重试不得生成新的 write token。
- 非成功错误体最多保留 4 KiB，去除控制字符并压缩空白；截断必须显式标记，任何
  API key、upload authorization 或任意长度的响应内容不得被无界带入错误。

### R2 固定 Zotero API 协议版本

- `ZoteroRemote` 的认证 request builder 对 GET 与所有写请求统一添加固定
  `Zotero-API-Version: 3`；该值作为 client 自身的固定配置保存，而不是散落在调用点。
- 外部 attachment upload 不得携带 API key 或 Zotero API version，保持已归档
  credential-boundary 契约。

### R3 OA PDF 不可信下载边界

- Auto attach 仅接受 HTTPS URL；每个初始/redirect URL 都必须重新解析，并拒绝
  userinfo、缺失 host、非默认/显式异常 scheme。redirect 最多 5 跳，禁止 HTTPS
  降级。
- 每一跳发送前解析 DNS；任一结果为 loopback、private、link-local、unspecified、
  multicast、documentation、carrier-grade NAT 或其他非公网地址时 fail closed。
  URL 中的 IP literal 使用相同策略。
- 禁用 reqwest 自动 redirect，逐跳校验 `Location`；不得把 Zotero credentials 或
  provider credentials 转发到下载目标。
- 先检查 `Content-Length`，再把 body 流式写入受 `NamedTempFile` 管理的临时文件；
  实际读取最多 100 MiB，超限、截断或读取失败自动清理。
- 最终 2xx 响应必须是 `Content-Type: application/pdf`（参数可忽略），且首个非空
  字节序列以 `%PDF-` 开头；不满足时返回稳定 typed error，绝不上传。
- `LinkedUrl` 模式只保存用户可见 URL，不发起下载；安全下载策略只约束 `Auto`。

### R4 附件上传资源上限与失败补偿

- 在创建 attachment item 之前检查本地文件为 regular file 且不超过 100 MiB；超限
  使用稳定 `attachment-size` 错误，并且不发送任何请求。
- 授权所需 MD5 以流式读取计算；上传 payload 允许单个有上限缓冲，但不得同时保留
  `file bytes` 与 `prefix + file + suffix` 两份完整缓冲。
- attachment item 创建成功后，授权、授权字段校验、外部上传或注册任一步失败，都
  必须 best-effort 硬删除新建 attachment item。原始错误保持主错误，hint/message
  明确记录 cleanup `succeeded` 或 `failed` 及失败原因，不能把清理失败伪装成成功。
- 若 authorization 返回 `exists=true`，保留成功短路；已完成注册的 attachment 不清理。
- 外部上传继续只允许生产 HTTPS，并继续证明不携带 Zotero API key/version。

### R5 arXiv Atom 使用结构化 XML 解析

- 使用 `quick-xml` streaming reader 定位 Atom `entry` 下的 title、summary、published
  与 author/name；正确处理 namespace、本体实体、CDATA、嵌套文本和多 author。
- feed title 不能误当 entry title；无 entry/title 或 malformed XML 返回稳定
  `arxiv-parse`，不得 panic 或回退 regex。
- `quick-xml` 是本任务唯一新增生产依赖，固定在 workspace dependencies 并由
  `zot-remote` 继承；不引入完整 DOM 或通用 feed abstraction。

### R6 兼容性与范围

- 既有成功响应模型、CLI attach mode、Zotero 写入前置条件和 origin-scoped credential
  契约保持不变。
- 不重试不带 write token 的变更请求，不实现通用代理/allowlist 产品配置，不改动
  父任务范围外的 application/use-case 层、observability 或 release provenance。

## Acceptance Criteria

- [ ] fake server 证明 GET 对 429/5xx 最多重试 3 次、尊重零秒 `Retry-After`，普通
      条件写只发送一次，write-token POST 重试时 token 字节完全相同。
- [ ] 错误体测试覆盖超长、控制字符与敏感样式内容，输出不超过约定上限且有截断标记。
- [ ] Zotero 所有 API fake-server 请求均含 `Zotero-API-Version: 3`；external upload
      同时不含 API key 和 version。
- [ ] OA URL/redirect 测试覆盖 HTTP、userinfo、loopback/private/link-local literal、
      私网 redirect、过多 redirect、非 PDF content type、错误 magic、声明/实际超限；
      任一失败均不调用附件上传。
- [ ] 合法 PDF 通过有界流式临时文件上传，成功/失败后均不遗留临时文件。
- [ ] 本地超限附件在零请求下失败；授权/上传/注册故障分别证明 orphan cleanup 成功，
      cleanup 自身失败时错误证据同时保留原始失败与 cleanup failure。
- [ ] arXiv fixture 覆盖 namespace/entity/CDATA/嵌套 author 内容与 malformed XML，且
      `oa.rs` 不再以 regex 提取 Atom 字段。
- [ ] `cargo test -p zot-remote`、相关 `cargo test -p zot-cli`、manifest guard 与最终
      `just ci` 全部通过；远程安全契约同步到 `.trellis/spec/`。

## Out Of Scope

- 用户可配置的通用 retry/SSRF allowlist、跨请求 circuit breaker 或持久化下载队列。
- 对 `LinkedUrl` 进行可达性探测，或限制用户显式传入的本地附件低于 Zotero 服务端
  已知额度之外的其他 MIME 类型。
- 重构所有 remote clients 为统一 trait、完整 streaming multipart abstraction，或
  父任务列出的长期架构项。
