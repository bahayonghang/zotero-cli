# Journal - lyh (Part 1)

> AI development session journal
> Started: 2026-06-10

---


## Session 1: Bootstrap Trellis Guidelines

**Date**: 2026-06-10
**Task**: Bootstrap Trellis Guidelines
**Package**: zot-core
**Branch**: `main`

### Summary

Filled project-specific Trellis specs for zot-core, zot-local, zot-remote, and zot-cli; verified with just ci.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `310662f` | (see git log) |
| `7feeb3b` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 2: Rename zot skill package

**Date**: 2026-06-10
**Task**: Rename zot skill package
**Package**: zot-core
**Branch**: `main`

### Summary

将 bundled skill 从 zot-skills 更名为 zot，同步 README/docs/AGENTS/eval 引用，并记录 zot-brainstorm 待办。验证通过 just ci 与 VitePress build。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `3d15e02` | (see git log) |
| `eaef1d7` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 3: Implement zot-brainstorm skill

**Date**: 2026-06-10
**Task**: Implement zot-brainstorm skill
**Package**: zot-core
**Branch**: `main`

### Summary

Created the Zotero reference-grounded brainstorming skill with Markdown/HTML report templates, regression prompts, evals, and documentation entry points.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `f1a34a1` | (see git log) |
| `a5c8007` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 4: 本地 Zotero 知识图谱与可视化分析

**Date**: 2026-06-26
**Task**: 本地 Zotero 知识图谱与可视化分析
**Branch**: `main`

### Summary

新增 zot graph / zot graph serve：从本地 zotero.sqlite 构建论文关系知识图谱（合著/共享标签/同collection/Zotero相关条目四类加权边），手写确定性分析（度中心性、并查集连通分量、标签传播社区、Top榜）。Slice A：zot-core 图类型 + zot-local graph.rs/db.rs（复用 search+get_items_batch，单条 itemRelations 查询），fixture 单测。Slice B：tiny_http 本地静态服务（127.0.0.1，端口回退，Ctrl-C，include_str! 内联资源）+ Cytoscape.js 前端（社区着色/搜索/权重·度过滤/节点详情，离线vendored）。验证 clippy 零问题 + 78 tests + 真实库 1538节点·16224边端到端 + server 全路由。Phase 2 待办：引文图谱(Semantic Scholar)、服务端实时重查、petgraph 介数/Louvain、多类型节点。子代理派发遇 429 故主会话内联实现。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `7ab712a` | (see git log) |
| `d4caef1` | (see git log) |
| `a0a11ee` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 5: library-seam 落地:窄 trait + AppContext 可替换,父任务 arch-deepening 收官

**Date**: 2026-07-08
**Task**: library-seam 落地:窄 trait + AppContext 可替换,父任务 arch-deepening 收官
**Branch**: `main`

### Summary

D1-D6 全按推荐定案(D6 被 rag-engine 落地自然消解:直接复用 RagLibrary,不另造 PdfSource)。五数据域窄 trait + Arc<dyn PdfBackend> 入 AppContext(生产构造 2 处),semantic search 与 collection/note 经 fake 测试,workspace 测试 129→140。父任务跨子任务 AC 全数核验(grep: if ctx.json=1/remote_err=1/PendingEmbedding=1/Pdfium 构造=2),真机冒烟 doctor/collection/related 通过,10/10 子任务归档。教训:commit-msg hook 现按 CJK 双列宽校验 72 列,subject 需更短;测试 expect_err 要求 CommandOutput 实现 Debug(已手写补上,后续 handler 测试直接可用)。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `a05e48a` | (see git log) |
| `1d11647` | (see git log) |
| `6499878` | (see git log) |
| `c0adb62` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 6: 完成重复条目安全清理与发布准备

**Date**: 2026-07-11
**Task**: 完成重复条目安全清理与发布准备
**Branch**: `main`

### Summary

完成 library dedupe、跨类型合并与引文保护，补齐中英文 safety 清单；拆分提交 0.6.0 发布准备和 Trellis 0.6.6 运行时同步。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `bb5df559a5eece49ded8cf50e4cdc2631a5dc429` | (see git log) |
| `dd36072ca0f10a241a230ca2fef3708d43221142` | (see git log) |
| `2407d12e14bb081118657a9d77bfbfa52177ab9c` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 7: 完成 Zotero 本机桥接基础

**Date**: 2026-07-11
**Task**: 完成 Zotero 本机桥接基础
**Branch**: `dev`

### Summary

实现并验证 Zot Bridge 插件、Rust desktop client、配对撤销、doctor 能力与真实 Zotero 9.0.6 smoke；补充协议规范并归档子任务。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `eedb83f` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 8: 完成本机 merge 与 dedupe 闭环

**Date**: 2026-07-11
**Task**: 完成本机 merge 与 dedupe 闭环
**Branch**: `dev`

### Summary

实现 desktop/web writer 边界、Zotero 原生 merge preview/apply、dedupe 低置信度门与幂等重试；修复同 profile XPI 重装后的连接持久化并验证 installed CLI。just xpi-check、just ci、diff/secret scan 通过；用户明确豁免隔离 profile 真实 merge smoke，未执行任何真实合并写入。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `52cdef8` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 9: 完成本机写入 skill 与文档对齐

**Date**: 2026-07-12
**Task**: 完成本机写入 skill 与文档对齐
**Branch**: `dev`

### Summary

更新 canonical zot skill 与 35 条路由评测，增加镜像漂移守卫，完成双语 desktop/Web 写入文档和 executable spec 对齐；iteration-2 安全断言 3/3，全部质量门通过。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `8d3affa` | (see git log) |
| `50f639e` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 10: 归档本机安全写入父任务

**Date**: 2026-07-12
**Task**: 归档本机安全写入父任务
**Branch**: `dev`

### Summary

三个第一阶段子任务均已完成并归档；完成父任务最终生命周期归档，无新增代码提交。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

(No commits - planning session)

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 11: connector 本机导入路径(子任务 1)

**Date**: 2026-07-18
**Task**: connector 本机导入路径(子任务 1)
**Branch**: `dev`

### Summary

在 zot-desktop 新增 connector client(ping/getSelectedCollection/import),wire zot item import 命令把 BibTeX/RIS 导入运行中 Zotero 当前选中 collection:格式自动判定、dry-run 复述目标与记录数、confirm 前强制 editable/library_editable 只读校验(只读目标绝不发 import),doctor 新增 connector_write 能力位(scope: import-only)。错误路径无 Web fallback,connector 模块不依赖 bridge 内部。just ci 全绿 217 测试;同步 executable spec(新增 connector.md,更新 error-handling/quality-guidelines/desktop-bridge/index)。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `3bfee8e` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 12: Remove zot-bridge and desktop write backend

**Date**: 2026-07-18
**Task**: Remove zot-bridge and desktop write backend
**Branch**: `dev`

### Summary

Removed the zot-bridge XPI, bridge CLI/config, desktop merge backend, and legacy JSON fields; merge/dedupe now use Web API only. Added root/profile legacy-config migration detection, moved local HTTP probing into ConnectorClient, and synchronized bilingual docs/specs. Verified just ci (217 tests), VitePress build, live read-only doctor, no-credential merge, and removed clap surfaces. Real credentialed merge --confirm remains unrun because this environment has no Web credentials and no real-library write authorization.

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `bae3544` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 13: Optimize zot connector skill

**Date**: 2026-07-18
**Task**: Optimize zot connector skill
**Branch**: `dev`

### Summary

重写 canonical zot skill 的 connector import 与 Web mutation 路由，同步 35 条 eval/test fixture 和 near-neighbor 负例；完成 just install、skills-check、just ci 与 diff 检查，归档子任务 3，保留父任务供最终集成复核。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `7fd1938` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 14: Finalize connector replacement parent

**Date**: 2026-07-18
**Task**: Finalize connector replacement parent
**Branch**: `dev`

### Summary

完成 07-18-connector-replace-bridge 父任务最终集成复核：just ci、VitePress build、skills-check、diff check、live doctor 与 connector dry-run 通过；真实 item import --confirm/只读实库目标和带 Web 凭据的 merge --confirm 仍为 missing evidence，未改写为通过且未写真实库。归档父任务，三个子任务记录保持完整。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

(No commits - planning session)

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 15: 完成 P0 Pdfium CWD 动态库劫持修复

**Date**: 2026-07-26
**Task**: 完成 P0 Pdfium CWD 动态库劫持修复
**Branch**: `dev`

### Summary

完成 07-26-fix-pdfium-cwd-rce：删除隐式 CWD 候选及 pdfium-render 裸库名 system fallback，改为可信来源回归测试，补齐 zot-local 可执行安全契约；聚焦测试、clippy 与 just ci 全部通过。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `982c012` | (see git log) |
| `c1e2d96` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 16: 完成凭据与 workspace 路径边界热修

**Date**: 2026-07-26
**Task**: 完成凭据与 workspace 路径边界热修
**Branch**: `dev`

### Summary

拆分 Zotero 认证与外部附件上传请求，强制生产上传 HTTPS；引入 WorkspaceName 并统一 TOML/RAG canonical containment，补齐回归测试与安全 code-spec，just ci 全绿。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `2da80f6` | (see git log) |
| `1f31505` | (see git log) |
| `242e99d` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 17: 完成 Pdfium 下载校验与原子安装

**Date**: 2026-07-26
**Task**: 完成 Pdfium 下载校验与原子安装
**Branch**: `dev`

### Summary

固定七平台双层 SHA-256 manifest，引入有界流式下载、regular-entry 解压、跨进程锁、同步原子发布和受管缓存复核，并通过聚焦测试与 just ci。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `f746f8d` | (see git log) |
| `6b65241` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 18: 完成 SQLite 一致快照整改

**Date**: 2026-07-26
**Task**: 完成 SQLite 一致快照整改
**Branch**: `dev`

### Summary

移除 live DB immutable 与手工 DB/WAL/SHM copy，改用只读源连接、限时分页 Backup API、quick_check、稳定 busy error 和 doctor 快照元数据，并通过并发 writer 回归与 just ci。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `08a5925` | (see git log) |
| `b1b7f50` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 19: 完成 JSON 错误协议统一

**Date**: 2026-07-26
**Task**: 完成 JSON 错误协议统一
**Branch**: `dev`

### Summary

统一 CLI 顶层 AppError 分类与 versioned error envelope，补齐 --verbose、Clap parse、graph serve/completions 独立协议和十个命令组单文档回归测试，并通过 just ci。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `1dc7e0b` | (see git log) |
| `f4810f7` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 20: 完成批量标签写入安全门禁

**Date**: 2026-07-26
**Task**: 完成批量标签写入安全门禁
**Branch**: `dev`

### Summary

完成 item tag batch 的本地预览、显式确认、影响上限、逐项失败隔离与结构化结果契约；同步 CLI、技能、双语文档和质量规范，并通过聚焦测试、技能镜像检查、Clippy 与 just ci。

### Main Changes

- Detailed change bullets were not supplied; see the summary above.

### Git Commits

| Hash | Message |
|------|---------|
| `f5681c9` | (see git log) |
| `33808ae` | (see git log) |

### Testing

- Validation was not recorded for this session.

### Status

[OK] **Completed**

### Next Steps

- None - task complete
