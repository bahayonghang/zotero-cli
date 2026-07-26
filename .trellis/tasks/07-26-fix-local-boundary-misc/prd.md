# P2: 本地边界与 sidecar 杂项加固

## Goal

关闭审计中五个相互独立但范围较小的本地边界缺陷：附件下载路径与覆盖策略、区域标注
数值约束、graph viewer 不可信 URL 与响应头、PDF 文本缓存的一致性/并发契约，以及
connector import 在当前 Zotero UI target 上的 TOCTOU 窗口。

## Background

- 审计报告 `zotero-cli-code-audit-2026-07-25.md:147-152` 与父任务映射确认本任务覆盖
  PDF cache、graph viewer、附件下载、区域标注和 connector target 五项；父任务将
  sidecar 结论校准为 PARTIAL，因为 `RagIndex` 已有 WAL/busy timeout，`PdfCache` 没有。
- `src/zot-cli/src/commands/item/read.rs:122-155,223-235` 将不可信
  `attachment.filename` 直接拼入输出目录，并以 `fs::copy` 默认覆盖目标文件；
  `ItemDownloadArgs` 当前没有 `--force`。
- `src/zot-cli/src/commands/item/annotation.rs:140-166` 与
  `src/zot-local/src/pdf.rs:423-474` 在加载 PDF 和构造远程 payload 前均未验证
  `x/y/width/height` 的有限性与单位矩形边界。
- `src/zot-cli/assets/graph/app.js:143-170` 以字符串拼接和 `innerHTML` 渲染 graph 数据，
  `node.url` 可进入任意 scheme，`target=_blank` 未设置 `noopener`；
  `src/zot-cli/src/commands/graph/server.rs:79-94` 未发送 CSP 或 `nosniff`。
- `src/zot-local/src/pdf.rs:476-535,1113-1126` 的 `PdfCache` 未启用 WAL、busy timeout
  或 schema version，cache key 仅为 path/mtime/length 的 MD5；
  `src/zot-local/src/workspace_rag.rs:28-38` 仍按既有契约共享 `.md_cache.sqlite`。
- `src/zot-cli/src/commands/item/import.rs:32-68` 在 confirm 分支只读取一次 selected target；
  `src/zot-desktop/src/connector.rs:47-65` 尚未保留响应中的 `libraryID`，无法完整区分
  跨 library 的 target 变化。

## Requirements

### R1 附件下载路径与 no-clobber

- 仅当 `--output` 缺省或指向现有目录时使用附件元数据文件名；该文件名必须是单个
  非空 basename，拒绝 absolute、`.`、`..`、`/`、`\\`、Windows prefix/ADS 风格冒号。
- 用户显式提供非目录 `--output` 时按该路径写入，不把附件元数据参与路径构造。
- `zot item download` 新增 `--force`。默认使用 OS 原子 `create_new` 打开目标，已存在时
  返回稳定 `attachment-exists` 且不修改原文件；仅 `--force` 才允许 truncate/overwrite。
- 复制失败必须保留 typed filesystem evidence；成功 envelope 与人类输出格式保持不变。

### R2 区域标注数值边界

- 在任何 PDF 加载和远程写入前验证 `x/y/width/height` 全部为 finite。
- 要求 `0 <= x,y < 1`、`width,height > 0` 且 `x + width <= 1`、
  `y + height <= 1`；边界值 0/1 按上述闭开区间处理。
- CLI orchestration 与 `PdfiumBackend::build_area_position` 复用同一个验证函数，避免
  非 CLI 调用绕过；失败返回稳定 `invalid-annotation-area`，不得产生 NaN/Inf JSON。

### R3 graph viewer URL 与浏览器边界

- graph 数据渲染不再通过 HTML 字符串拼接；详情、tags、community legend 使用
  `textContent`、`createElement`、`append` 等 DOM API。
- `node.url` 只有解析为绝对 `http:` 或 `https:` URL 时才渲染为可点击链接；其他
  scheme/相对 URL 仅显示为普通文本。DOI 由固定 `https://doi.org/` origin 构造，
  Zotero 链接仅由 item key 构造。
- 所有新窗口链接设置 `target=_blank` 与 `rel="noopener noreferrer"`。
- graph server 的成功资源与 404 均添加 `Content-Security-Policy`、
  `X-Content-Type-Options: nosniff`；CSP 至少包含 `default-src 'self'`、
  `script-src 'self'`、`object-src 'none'`，并仅为现有内联 CSS/样式放行 style。

### R4 PDF cache sidecar 契约

- `PdfCache::new` 对每个文件型 cache 统一启用 WAL 与 5 秒 busy timeout，并设置显式
  `PRAGMA user_version`。拒绝高于当前实现的 schema version，旧的未版本化 cache
  就地升级且不破坏可读取数据。
- 保持默认 cache、library semantic cache 与 workspace 共享 `.md_cache.sqlite` 的现有
  路径布局；本任务不拆分或迁移用户 sidecar 文件。
- cache key 使用流式 SHA-256 内容摘要并带算法前缀；同路径、同长度、同 mtime 的内容
  替换不得命中旧文本。旧 MD5 key 自然失效，不需要全表迁移。
- SQLite open/schema/get/put 错误继续映射为 `ZotError::Database` 的稳定 code。

### R5 connector target TOCTOU

- `SelectedTarget` 保留 `libraryID`，并将 library、collection id/name、editable 与
  library editable 组成语义指纹。
- dry-run 仍只执行一次 selected-target 请求且绝不 import；confirm 分支在解析输入后、
  import 前立即再次获取 target。
- 第二次 target 与第一次不同，或已不可写时 fail closed，返回稳定
  `connector-target-changed`/`connector-target-readonly`，不得发送 import 请求；提示用户
  回到 Zotero 确认选择后重新 preview/confirm。
- confirmed success envelope 报告实际复验过的 target，既有 session/status/entry 语义不变。

### R6 兼容性与范围

- 不改变 Zotero SQLite 只读边界、Web API 写入边界、workspace 文件布局、PDF 提取模型
  或 graph 数据模型。
- 不实现跨进程 preview token、通用下载事务、全局 Web CSP 框架或长期 application
  layer；connector API 无法原子绑定 target 的剩余竞态必须保持明确。

## Acceptance Criteria

- [ ] 路径测试覆盖 `../`、absolute、Windows separator/prefix/ADS、空 basename 与正常
      filename；显式 output file 不受元数据 filename 影响。
- [ ] 下载测试证明默认已存在目标返回 `attachment-exists` 且原内容不变，`--force`
      才覆盖；CLI parse 覆盖新 flag。
- [ ] 区域验证覆盖 NaN、正负 infinity、负坐标、零/负宽高、起点/终点越界与合法边界，
      invalid case 在 PDF/remote I/O 前失败。
- [ ] graph asset 不再用 `innerHTML` 渲染不可信 graph 字段；URL policy 覆盖
      `javascript:`、relative、http/https，所有外部链接带 `noopener noreferrer`。
- [ ] graph server route 测试断言 HTML/JS/JSON/404 均有 CSP 与 nosniff，资源 MIME
      与既有状态码保持正确。
- [ ] PdfCache reopen 测试断言 WAL、5000 ms busy timeout、schema version；固定同一
      mtime/length 后替换内容不命中旧 cache，正常 put/get 仍通过。
- [ ] connector fake server 证明 dry-run 仍为两次总请求，confirm 使用两次 target read；
      library/collection/writability 变化时零 import，稳定 target 才成功。
- [ ] `cargo test -p zot-local`、`cargo test -p zot-desktop`、相关
      `cargo test -p zot-cli`、`task.py validate` 与最终 `just ci` 全部通过；可复用契约
      同步到 `.trellis/spec/` 和必要的 operator limits/docs。

## Out Of Scope

- 为 connector import 引入跨进程签名 plan/token，或修改 Zotero connector server。
- 对用户显式 `--output` 路径施加 workspace containment，或实现通用原子文件发布层。
- 拆分共享 `.md_cache.sqlite`、迁移旧 cache rows、缓存 Zotero attachment md5，或新增
  sidecar 管理服务。
- 重写 graph viewer 视觉设计、替换 Cytoscape、允许用户配置 URL scheme/CSP。
