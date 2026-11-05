# 本地标签增删与安全回读

## Goal

补齐 Agent 最直接的本地整理缺口：不配置 Web key，也能给单条文献添加或移除指定标签。

## Background

归属父任务 R1、R4、R5、R7。现有 `item tag add/remove` 调用 `ctx.remote()`（`src/zot-cli/src/commands/item/tag.rs:26`）；`LocalWriteArgs` 已提供 local 选择和实例/版本确认（`src/zot-cli/src/cli/args.rs:539`）。

## Requirements

- T1：复用现有 add/remove 命令，新增显式 local 后端；不带后端仍保持 Web 行为。
- T2：默认预览，展示 before/after 和实际变更；确认绑定预览的实例与条目版本。
- T3：新增标签采用手工标签语义；同名已存在则不重复添加，也不改变其原有类型。移除按精确名称移除该名称的标签关联；保留其余标签对象。
- T4：不做大小写折叠、同义词合并或自动修剪非空名称；纯空白输入拒绝。重复请求去重，重复执行返回 unchanged。
- T5：仅改目标条目的 tags；不使用库级删除标签接口；不改其他条目、成员、字段或 children。
- T6：沿用本地普通条目边界和真实授权流程；不能借 field 更新入口绕过标签契约。

## Acceptance Criteria

- [x] T-AC1：新增/移除中文、空格、特殊字符标签可预览、确认、回读；新标签类型正确。
- [x] T-AC2：保留自动标签 type=1、手工标签 type=0/省略以及所有未被移除的标签信息；回读顺序不同不误报失败。
- [x] T-AC3：重复添加、移除不存在标签为 unchanged，不读凭据、不授权、不 PATCH。
- [x] T-AC4：空白输入、非法 key、子条目、回收站条目在提交前拒绝；冲突/拒绝/401/超时/回读失败沿用本地状态契约。
- [x] T-AC5：Web 默认路由及现有批量标签行为保持兼容；标签 local 路径不调用 SQLite 或 Web。
- [x] T-AC6：隔离真实库中新增、移除、重复操作、UI 确认与重启回读通过；测试、文档、skill 同步完成。

## Out of scope

批量执行（交子任务 C）、标签重命名/颜色/词表清理、标签类型转换、子附件/笔记标签、本地批量标签旧命令扩展。

## Dependencies / status

独立于 B；为 C 提供可复用的标签差异计算与条件写入能力。review：实现、自动化和隔离桌面核心验收通过，待提交收口。

## 本轮收口状态

2026-11-02 核心实现与验收完成，进入 review 待提交收口。Windows Zotero 10.0.7 独立 profile/data：标签增删、根/子分类创建、3 条不同决策批量 apply、API/UI/正常重启回读通过；原实例已恢复。173 个 CLI 测试及 workspace/质量门通过。故障注入与真实桌面证据分列；用户要求聚焦核心，不扩展桌面故障测试。未 commit/push/PR，未归档。见父 validation.md。

AC 勾选表示对应实现及指定证据通过，不表示所有故障都在真实 Zotero 重演。组库、其他 OS、真实提交中断与 UI 制造版本漂移未实测，详见父 validation.md。
