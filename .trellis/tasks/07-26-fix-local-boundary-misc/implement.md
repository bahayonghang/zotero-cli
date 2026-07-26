# Implementation Plan: 本地边界与 sidecar 杂项加固

## 1. 附件下载

- [ ] 在 `ItemDownloadArgs` 添加 `--force` 并更新 CLI parse fixture。
- [ ] 在 `item/read.rs` 添加跨平台 basename 验证和 typed `attachment-exists`。
- [ ] 用 `OpenOptions + io::copy` 实现 default create-new / explicit overwrite。
- [ ] 补路径与 no-clobber/force helper 测试。

## 2. 区域标注

- [ ] 在 `zot-local/pdf.rs` 增加共享纯验证函数与边界表测试，并从 `lib.rs` 导出。
- [ ] CLI 在 local/PDF/remote I/O 前调用验证；Pdfium backend 加防御性复验。

## 3. Graph viewer

- [ ] 将 detail、tag、legend 的 graph-derived HTML 改为 DOM API。
- [ ] 添加 HTTP(S)-only URL helper、固定 DOI/Zotero link 与 `_blank` rel。
- [ ] server 为所有 route 加 CSP/nosniff/referrer policy，并补 route header/status 测试。

## 4. PDF cache sidecar

- [ ] `PdfCache::new` 设置 WAL、5000 ms busy timeout、schema user_version 与未来版本拒绝。
- [ ] 将 cache key 改为流式 SHA-256 内容指纹和算法前缀。
- [ ] 补 reopen PRAGMA、正常命中、固定 mtime/length 替换失效及 future schema 测试。

## 5. Connector target

- [ ] `SelectedTarget` 保留 `libraryID` 并更新解析测试/规范。
- [ ] confirm 分支第二次读取并比较完整 target fingerprint，变化/readonly 均在 import 前失败。
- [ ] 调整 scripted fake-server 请求数，补跨 library/collection/writability 变化零写入测试。

## 6. 契约与验证

- [ ] 更新 `.trellis/spec/zot-local` 的 PDF cache/annotation 契约。
- [ ] 更新 `.trellis/spec/zot-cli` 的 download/graph/connector 契约及必要 operator limits。
- [ ] 运行 `cargo fmt --all`。
- [ ] 先运行 `cargo test -p zot-local`、`cargo test -p zot-desktop`、聚焦
      `cargo test -p zot-cli` 与 clippy。
- [ ] 运行 `python ./.trellis/scripts/task.py validate ...`、`git diff --check` 和真实门禁
      `just ci`。
- [ ] 逐项勾选 PRD acceptance，按中文 emoji `[AI]` 原子提交，归档并记录 journal。

## Risk / Rollback Points

- `src/zot-local/src/pdf.rs` 同时包含 Pdfium 下载与 cache；编辑只限区域验证和 PdfCache，
  不触碰已归档 P0/P1 的可信加载/下载校验路径。
- `app.js` 没有 JS 单测 runner；以无 `innerHTML`/scheme policy 的静态断言和 server route
  测试补足，最终通过 embedded asset 编译验证。
- connector fake server 的脚本响应数必须与 preview/confirm 新流程严格一致，避免挂起；
  每个 failure fixture 只提供预期请求，意外 import 会直接失败。
- 不清理旧 PDF cache rows，不改变 `.md_cache.sqlite` 路径，回滚无需数据迁移。
