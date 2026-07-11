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
