# 执行计划

1. 用户批准后复核父任务能力证据、最新版本与入口；按 R1/R2 校正 AGENTS.md 和 issue-tracker。
2. 添加共享矩阵和必要 CLAUDE.md 薄入口；其他专用入口仅在当前官方证据要求时添加。
3. 更新中英用法页与 README 链接，修正 config 注释与 workflow_phase docstring但保留 inline/执行逻辑；写 Grok/Kimi 的手动通用步骤及 session identity 说明。
4. 若改 canonical skill，有意执行 just skills-sync；不手改镜像。C2 未完成时显式检查两个 skill。
5. 人工沿每个入口核验共享规则与授权边界；执行 cargo test -p zot-cli commands::item::import --locked、cargo test -p zot-desktop --locked、cargo test -p zot-cli --test json_error_contract --locked，最后 just ci、git diff --check。
6. 在可用五工具会话中只读验证规则来源、planning 边界和 doctor 路径；记录工具版本和证据。不可用项标 UNVERIFIED，不靠 grep 代替。
7. 强模型复核 AC1–AC4，回写适用工具与验收记录；未获得发布或全局写授权不做这些动作。

便宜模型任务包须含精确文件、批准文案、检查命令；新冲突交回强模型。docs 构建依赖 C3 的可用构建环境，未准备时记录缺口。

## 精确执行文件清单
- AGENTS.md：运行事实、目录、测试布局、PRD 权威和工具入口。
- CLAUDE.md（新增）：最小 @AGENTS.md 薄入口。
- docs/agents/harnesses.md（新增）：五工具矩阵、Grok/Kimi 手动 inline 和证据层级。
- docs/agents/issue-tracker.md：local PRD / remote issue 关系。
- README.md、README.zh-CN.md：共享说明入口，使用同一行为承诺。
- docs/skills/agent-usage.md、docs/en/skills/agent-usage.md：工具范围和用法链接。
- skills/zot/SKILL.md：平台路径与已证实语义事实；不修改 zot-brainstorm。
- docs/guide/getting-started.md、docs/en/guide/getting-started.md：平台路径说明。
- docs/cli/config.md、docs/en/cli/config.md：实际 config 目录机制。
- docs/cli/workspace.md、docs/en/cli/workspace.md：实际 workspace 目录机制。
- docs/agents/limits.md：校正同类通用路径表述。
- .trellis/config.yaml、.trellis/scripts/common/workflow_phase.py：仅同步 Codex inline 理由的注释/docstring，不改配置值/运行逻辑。
- .agents/skills/zot、.claude/skills/zot：仅由 skills-sync 有意再生成，不手改；两套 zot-brainstorm 镜像可能被相同同步命令重拷贝，内容应保持原样。

收尾逐文件核对上述清单；合理 Linux 示例只补平台标签，不机械删除。文件外新发现先交强模型判断是否属于已批准范围。
