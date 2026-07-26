# Design: 本地边界与 sidecar 杂项加固

## 1. 边界与所有权

- `zot-cli/item/read.rs` 拥有 download 的用户路径意图与 no-clobber 文件创建。
- `zot-local/pdf.rs` 拥有区域几何约束和 PDF cache 的 SQLite/指纹契约；CLI 复用其纯验证
  函数，在构造 `AppContext` 依赖后的最早业务位置 fail fast。
- graph browser 侧拥有 DOM/URL policy，localhost server 侧拥有响应安全 header。
- `zot-desktop::SelectedTarget` 保存 connector 原始 target identity；`zot-cli/item/import.rs`
  拥有 confirm 前复验与稳定错误语义。

## 2. 附件下载

`safe_attachment_basename(filename)` 只接受单个普通组件，并额外按跨平台规则拒绝 `/`、
`\\` 与 `:`。`resolve_download_path` 仅在 output 缺省/为目录时调用它。复制使用
`OpenOptions`：默认 `create_new(true)`；`--force` 使用 `create(true).truncate(true)`；随后
`io::copy` 从 source 到目标。`AlreadyExists` 单独映射为 `attachment-exists`，消除
exists-check + copy 的竞态。

## 3. 区域标注

新增纯函数 `validate_area_coordinates(x, y, width, height) -> ZotResult<()>`，先检查
`is_finite`，再检查单位矩形约束。CLI 在获取 local attachment 前调用；Pdfium backend
在加载 library/document 前再次调用。两层共享同一实现，不复制条件或错误文案。

## 4. Graph viewer

使用小型 DOM helper 创建 field、text、link 与 tag。`safe_web_url` 通过浏览器 `URL`
解析器只接受绝对 HTTP(S)；不合格值保留可见文本但不可点击。DOI 和 Zotero link 的
origin/scheme 由代码固定，所有 `_blank` link 通过同一 helper 附加 rel。

server 通过单一 `secure_response` helper 给所有 route 响应增加：

```text
Content-Security-Policy: default-src 'self'; script-src 'self';
  style-src 'self' 'unsafe-inline'; img-src 'self' data:;
  object-src 'none'; base-uri 'none'
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
```

`unsafe-inline` 仅限 style，因为现有 `index.html` 与 legend swatch 使用内联样式；脚本
不放行 inline/eval。

## 5. PdfCache

`PdfCache::new` 的顺序为：open -> busy timeout -> WAL -> 读取 `user_version` -> 拒绝未来
版本 -> 创建兼容 schema -> 设置当前版本。当前版本为 1；version 0 的既有表无需数据
重写。cache key 为 `sha256:<hex>`，使用现有流式 `sha256_reader` 读取文件内容。旧的
32 字符 MD5 key 不会与新 key 相撞，允许后续自然淘汰。

共享 `.md_cache.sqlite` 路径保持不变：修复的是共享 sidecar 的并发与失效语义，而非
引入路径迁移。WAL/busy timeout 每次连接都设置；schema version 持久化在 DB header。

## 6. Connector 复验

`SelectedTarget` 新增 serde 映射的 `library_id`。CLI 以
`(library_id, id, name, editable, library_editable)` 为 fingerprint。流程为：

```text
ping -> target A -> readonly gate -> parse/read input
  -> if preview: return A
  -> target B -> compare fingerprint -> readonly gate -> import -> report B
```

第二次 target 先复验 writability：变为 readonly 返回既有 `connector-target-readonly`；
仍可写但 identity 变化返回 `connector-target-changed`。两类失败都不 import。此方案
缩短但不能消除 second target read 与 connector import 之间的上游竞态；跨进程绑定 token
属于范围外。

## 7. 兼容性与回滚

- 新 `--force` 是向后兼容的 CLI 增量；默认行为从覆盖改为 no-clobber，属于刻意的安全
  收紧，错误使用稳定 envelope 表达。
- `SelectedTarget` JSON 增加 connector 原生命名的 `libraryID` 字段；既有字段与
  confirmed/dry-run envelope
  保留。
- cache schema/version 与 key 可回滚代码，但新 key rows 对旧代码只表现为 miss；无需
  destructive migration。
- graph headers/DOM 不改变 graph JSON；回滚不涉及持久数据。
