# 执行计划（2026-09-28 已批准）

## Gate
用户已批准 C1–C4；先复核 HEAD、工作区、批准项，再按实际进展启动子任务，保留其他变化。审查基线报告保留为修复前记录；执行结果另写 execution-report.md。

## Ordered Work
1. C1：强模型核对 TLS/MSRV，最小锁文件补丁；执行子任务规定的完整依赖检查。
2. C2：先建立可检出三平台空正文的正式测试，证明修前红；局部别名修复，五平台和模式检查转绿。
3. C3：据 C2 结果更新说明，显式 Codex inline，修正双语合并文案，记录发现/实际运行证据和成本边界。
4. C4：增加周度安全触发，仅 security 在 schedule 执行；更新验证契约。
5. 集成：just ci、npm --prefix docs run build、actionlint、cargo audit、cargo deny check、git diff --check；另完成 C1 的 MSRV/udeps/machete。无相关新变化的已通过检查不重复。
6. 强模型逐条核对 R1-R7、子 AC、改动文件、长期写回与未验证项。

## Controls
不以 fmt-fix/version-sync/skills-sync 修复未授权范围；当前计划不改 canonical skill，不需 skills-sync。不启动付费客户端会话，不改全局模型，不操作远端。缺失验证工具按批准的固定版本准备，不改变全局默认工具链；不降低 gate。

## Completion
每项 AC 有证据；新增结果绑定 SHA、时刻、命令和退出码。只有批准范围内 diff。实际 hosted、新会话和有副作用工作流未执行就标 UNVERIFIED。提交、归档、远端交付另按当时授权处理。
