# C2 / P1：修复五套平台上下文路由

## Goal
平台ID与工作流显示名一致映射；支持的入口必须返回有效步骤内容。

## Evidence
workflow_phase.py:135-142仅移除分隔符再比较。workflow.md:475-526使用Claude Code/Kimi Code/Oh My Pi；本机continue/start使用claude/kimi/omp。父任务research/platform-probe.json记录三者exit0但只有45字符标题；codex/grok正常。别名缺失是已复现原因。

## Requirements
- C2-R1：claude/Claude Code、kimi/Kimi Code、omp/Oh My Pi映射同一平台；保留既有空格/横线/下划线/大小写兼容。
- C2-R2：保留Codex auto/inline/sub-agent别名、非法值回退、Grok和无关平台现状；不改分派授权。
- C2-R3：正式回归进入现有Python测试发现，覆盖真实get_context CLI正文；未来Trellis更新后可检出复发。

## Acceptance Criteria
- C2-AC1 / R1：三平台短ID与显示名返回相同有效实施内容，不只标题；五平台2.1/2.2均有正确实现/检查要求。
- C2-AC2 / R2：Codex无配置/auto→sub-agent，显式inline→inline，legacy sub-agent→auto，无效配置→inline；Grok及至少一个非目标平台保持现有语义。
- C2-AC3 / R3：新增测试修前失败、修后通过；python -m unittest discover -s scripts/tests -p test_*.py 与just ci通过；测试不改变任务状态、全局配置或源文件。

## Scope
写 .trellis/scripts/common/workflow_phase.py 和新增 scripts/tests/test_harness_platform_routing.py。读取 workflow.md、cli_adapter.py、git_context.py、本地适配。禁止靠改五份生成命令、删除平台分支或增加默认权限规避。

## Authority and Dependency
2026-09-28 已批准，可独立实施；C3随后根据结果更新说明。主模型负责边界/回归设计，低成本执行者可做已冻结别名与测试。适用全部五套harness。
