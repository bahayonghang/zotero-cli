# C3 / P1：五套harness项目契约对齐

## Goal
共享规则、显式配置、实际发现和用户能力说明一致，避免错误分派与后端承诺。

## Evidence
- AGENTS.md:90-95与harnesses.md:35-36/79-93称Grok/Kimi无marker；workflow.md:223/475-526已包含两者。
- harnesses.md:30-34/60称Codex inline；config.yaml:129-145未启用该键，workflow_phase.py:161-176默认auto。
- Grok1.0.42的去敏inspect发现ignored .agents/skills/zot和zot-brainstorm，旧矩阵过滤ignored的一概说法不符本机。
- README.md:15和README.zh-CN.md:15宣称Desktop合并；item/merge.rs:125-126只选择WebMergeWriter。

## Requirements
- C3-R1：AGENTS共享事实、CLAUDE薄入口、单份五工具矩阵；不复制五套完整规则。
- C3-R2：明确设置Codex inline以落实项目已声明的默认；把能力支持与本次分派授权分开。
- C3-R3：注明五工具的规则发现、skill、模型交接、权限、当前证据等级；更新版本日期，撤回过时绝对结论。
- C3-R4：双语首屏说明merge/dedupe实际走Web API；保留connector import-only边界。
- C3-R5：注明强模型/低成本执行范围和长期写回适用工具；镜像相等不代表fresh-start成功。

## Acceptance Criteria
- C3-AC1 / R1-R3：共享说明与C2结果、显式config一致；根CLAUDE.md保持@AGENTS.md；Codex有效平台为inline。
- C3-AC2 / R3-R5：五工具均有官方/仓库/本机发现/未验证栏；不能声称Kimi低价子代理或OS sandbox已实测；Grok的skill model字段不被作为降档方法。
- C3-AC3 / R4：两份README首屏与WebMergeWriter及canonical skill一致。
- C3-AC4：C2正式测试、just ci、VitePress build、git diff --check通过；强模型复核全体说明，无计划被写成运行事实。

## Scope and Authority
依赖C2。写AGENTS.md、docs/agents/harnesses.md、.trellis/config.yaml、README.md、README.zh-CN.md。CLAUDE.md、canonical skills、ignored平台目录只读。新的通用skill、全局模型配置、客户端付费会话和团队知识库不在本轮实施范围。五工具共享说明；inline键仅Codex。
