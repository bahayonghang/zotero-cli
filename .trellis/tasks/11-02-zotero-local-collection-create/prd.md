# 本地分类创建与层级校验

## Goal

Agent 选定分类名称和已有父分类后，可通过本机 Zotero 创建分类并拿到明确 key，供后续成员操作使用。

## Background

归属父任务 R2、R4、R5、R7。`src/zot-cli/src/commands/collection.rs:10` 目前只走远端 create；已存在对象的成员更新不能直接解决“尚不存在的分类”的确认与重试问题。

## Requirements

- C1：复用 collection create，显式 local；支持根分类和同库已有父分类，不自动创建多层路径。
- C2：预览显示实例、库、精确名称、父分类及目标层级；确认时验证同一目标和本地库版本。
- C3：同一父级下出现精确同名分类时拒绝创建，列出候选 key；不擅自复用、合并或重命名。不同父级可同名。
- C4：空白名称、父分类不存在/已删除/不属于当前库、无权限、实例改变均明确失败。
- C5：成功必须取得服务端分类 key 并回读名称/父级一致。HTTP 200 本身不等于该对象创建成功。
- C6：提交结果未知时不自动重发或通过同名对象猜测成功。报告保留核查线索，避免重复创建。
- C7：现有 Web create 的参数与默认行为保持兼容。

## Acceptance Criteria

- [x] C-AC1：无 Web 凭据可预览并创建根分类及子分类，API/UI 名称、父级、key 一致。
- [x] C-AC2：同级同名拒绝、不同父级同名允许；预览后新增同名分类或库版本变化在提交前/条件提交时拦截。
- [x] C-AC3：本地库版本不与 item version 或 Web version 混用；根分类创建也有有效条件保护。
- [x] C-AC4：对象级 failed、错误身份、超时、5xx、畸形响应、回读失败分别正确报告，无盲重试。
- [x] C-AC5：创建所得 key 可交给现有本地 add-item；成员、children 与附件未被创建流程改动。
- [x] C-AC6：既有 Web/connector 回归、文档/skill、独立测试库真实授权与 UI 验收完成。

## Out of scope

分类重命名、删除、重挂父级、同名自动合并、多级树一次创建、批量分类创建。

## Dependencies / status

沿用原有 Local API 基线。与 A 无功能依赖；按 A → B 顺序实施。review：实现、自动化和隔离桌面核心验收通过，待提交收口。

## 本轮收口状态

2026-11-02 核心实现与验收完成，进入 review 待提交收口。Windows Zotero 10.0.7 独立 profile/data：标签增删、根/子分类创建、3 条不同决策批量 apply、API/UI/正常重启回读通过；原实例已恢复。173 个 CLI 测试及 workspace/质量门通过。故障注入与真实桌面证据分列；用户要求聚焦核心，不扩展桌面故障测试。未 commit/push/PR，未归档。见父 validation.md。

AC 勾选表示对应实现及指定证据通过，不表示所有故障都在真实 Zotero 重演。组库、其他 OS、真实提交中断与 UI 制造版本漂移未实测，详见父 validation.md。
