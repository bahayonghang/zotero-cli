# 设计

## 文件与权威
AGENTS.md 保存短项目事实；新增 docs/agents/harnesses.md 保存带日期、版本、来源和证据等级的五工具矩阵；新增 CLAUDE.md 用原生 @AGENTS.md 导入。其他工具优先使用经核验的 AGENTS.md 自动发现；不为对称而复制五份规则。

修改文件：AGENTS.md、CLAUDE.md、docs/agents/harnesses.md、docs/agents/issue-tracker.md、README.md / README.zh-CN.md 的入口链接、docs/skills/agent-usage.md、docs/en/skills/agent-usage.md、.trellis/config.yaml 注释与 .trellis/scripts/common/workflow_phase.py 的同类 docstring（仅文字）。canonical skills/zot/SKILL.md 只改已证实冲突的对应事实和共享文档引用；完整文件表见 implement.md。不得手改镜像、扩展安装根或改 Trellis managed block。

## 需求机制
R1 对照 Rust 调用链校正文案，不改变运行行为；目录说明涵盖 Linux/XDG、Windows、macOS。R2 在 issue-tracker 说明 local PRD 的执行权威与 remote issue 的协作边界。R3 以父任务 research/harness-review.md 的 primary-source 证据为准，能力、仓库适配、实际运行分列。R4 改为本项目采用 inline 的选择，不声称 fork 永远不能继承上下文；Grok/Kimi 直接读取 workflow.md 的通用步骤、显式 task path、读取对应 spec，在批准后按 inline 执行；不要用未匹配的平台 marker 假称已注入上下文。若缺少 session identity，按现有 TRELLIS_CONTEXT_ID 机制在当前进程提供唯一身份（不写用户全局环境），不扩展平台探测注册表。R5 修改中英文入口页并引用共享矩阵，不重写整个产品 skill。

## 验收机制
本轮路径全文检索确定同类文案的精确补充文件：skills/zot/SKILL.md、docs/guide/getting-started.md、docs/en/guide/getting-started.md、docs/cli/config.md、docs/en/cli/config.md、docs/cli/workspace.md、docs/en/cli/workspace.md、docs/agents/limits.md。这里只校正把 Linux 路径写成通用路径的内容；合理的 Linux 示例保留并标明平台。skills/zot-brainstorm/SKILL.md 当前未检出该路径，不在修改清单；以 implement.md 的精确清单为本次批准范围。

AC1 对照 Rust、doctor 摘要与 tests 文件审查；AC2 对照 PRD/issue 说明审查；AC3 每工具记录官方资料、实际版本、本地入口、新会话结果；AC4 人工核对中英说明并运行门禁。静态入口存在不等于运行时加载成功。无法启动的工具保留 UNVERIFIED，不宣称五工具全部实测对齐。

## 分工与回退
Claude Code/Codex 强模型负责共享规则优先级；Grok Build/Kimi Code/OMP 的原生入口由对应可用强模型核对。便宜模型执行批准文案替换、薄入口和翻译，强模型复核。撤销本子任务文档即可回退，不改变用户配置。此任务独占 canonical skill 文案；与 C2 共用 README 时只改入口段，C2 改验证段。
