# 五套 harness 规则与能力边界审查

- 审查日期：2026-09-07
- 范围：Claude Code、Codex、Grok Build、Kimi Code、Oh My Pi（OMP）的项目规则入口、skills/hooks/MCP/subagent/权限边界，以及本仓库的静态适配。
- 证据等级：本报告验证了仓库文件和官方文档；没有启动五套客户端做新会话握手，因此“实际启动时已发现/已启用”一律 `UNVERIFIED`。主线程随后只读探测了版本：Claude 2.1.263、Codex CLI 0.153.4、Grok 1.0.22、Kimi 0.41.0、OMP 18.1.12；见 local-validation.md。版本存在不代表当前官方资料中的每项能力已在该安装版本生效。
- 严重性口径：本轮未发现会直接导致服务阻断、数据损坏或安全事故的 P0；规则入口、事实语义和明确路由缺口列为 P1，防漂移检查覆盖不足列为 P2。

## 结论与优先级发现

### P1：Claude Code 在干净克隆中看不到项目主规则

Claude Code 官方明确只读 `CLAUDE.md`/`.claude/CLAUDE.md`，不会自动读 `AGENTS.md`；兼容做法是在根 `CLAUDE.md` 中 `@AGENTS.md`。本仓库只有被跟踪的 `AGENTS.md`，根 `CLAUDE.md` 缺失；已有 `.claude/` 又被 `.gitignore:6` 整体忽略，因此当前机器上的 hooks、agents 和 skill 镜像不属于可移植仓库契约。`AGENTS.md:94-107` 却把 Trellis 说成面向所有 AI 助手，并把 `.agents/skills`、`.codex/agents` 写成“可能存在”，这不足以让 Claude Code 获得规则。

候选最小修复：只新增被跟踪的根 `CLAUDE.md`，正文保留 `@AGENTS.md`；本需求不改 `.gitignore`，也不提交个人 `.claude/settings.json`、hooks 或 agents。检查：`git check-ignore -v CLAUDE.md` 应证明该文件可跟踪，Claude 新会话用 `/memory` 或等价的已加载指令视图证明根入口生效，最后运行 `just ci`。

### P1：项目说明已有两个真实运行时事实冲突

1. `AGENTS.md:40` 声称所有 library mutation 都走 Web API；但 canonical skill 明确把 `item import` 作为 connector 的唯一写路径（`skills/zot/SKILL.md:15-16,135-146,162-178`），代码也声明 connector 可向 Zotero UI 当前选中目标导入新记录（`src/zot-desktop/src/connector.rs:1-5`）。应把 AGENTS 改为“除 connector import 外的 mutation 走 Web API”。
2. `AGENTS.md:45,58`、多份 docs 和 skill 把配置/工作区写死为 `~/.config/zot/...`；代码以 `dirs::config_dir()` 计算平台目录（`src/zot-core/src/config.rs:184-195`，workspace 见 `src/zot-local/src/workspace.rs:1053-1054`）。本轮实际 doctor 在 Windows 返回 `C:\Users\lyh\AppData\Roaming\zot\config.toml`。应以 `doctor.data.config_file`/`config show` 为权威，并分别给 Linux/macOS/Windows 示例，避免五套 harness 在 Windows 按错误路径排障。

检查：相关 Rust 单元测试；Windows `zot --json doctor`；至少对说明/skill 做路径全文检索；`just ci`。这两项先由强模型审查语义边界，机械替换和镜像同步可交给便宜模型。

### P2：skill 的 canonical/mirror 所有权清楚，但验证只覆盖了一半

canonical 源是 `skills/`；`justfile:58-70` 把所有 skill 复制到忽略的 `.agents/skills` 与 `.claude/skills`。这与 Codex 的 `.agents/skills` 和 Claude 的 `.claude/skills` 入口一致。当前两套 `zot`/`zot-brainstorm` 镜像均一致，不能把它们标为 drift。

缺口在检查器：`scripts/check_skill_mirrors.py:51,63` 默认只比较 `skills/zot`，而仓库已有第二个 canonical skill；`justfile:74-76` 因此会漏掉 `zot-brainstorm` drift。现有测试只覆盖单 canonical tree（`scripts/tests/test_check_skill_mirrors.py:13-80`）。候选修复是让默认检查枚举 `skills/*/SKILL.md` 并逐个核对两个目标，保留显式单-tree 参数用于聚焦诊断。检查：新增“第二 skill 漂移导致默认命令非零”“全部未安装仍允许 skip”“只缺一套/一个 skill 失败”测试；`python scripts/check_skill_mirrors.py --skip-if-all-missing`；`python -m unittest discover -s scripts/tests -p "test_*.py"`；`just ci`。

### P1：Trellis 没有说明 Grok/Kimi 的手动 inline 路径，且 Codex 注释已过时

`.trellis/workflow.md:186,223-240,273-289,356-370` 列出 Claude Code、Codex 与 Oh My Pi，但没有 Grok Build 或 Kimi Code；`.trellis/scripts/common/task_store.py:116-136` 的 subagent 目录探测有 `.claude`/`.omp`，没有 `.grok`/`.kimi-code`。这只能证明 Trellis 没有为两者提供明确的自动探测与 dispatch 契约；不能证明手动 inline 流程不可行，也不能据此认定 harness 原生能力不可用。当前缺口是项目说明没有告诉 Grok/Kimi 操作者如何显式选择 inline 并带入 task path/context。

`.trellis/config.yaml:100-107` 与 `.trellis/scripts/common/workflow_phase.py:144-168` 又把 Codex 默认 inline 的理由固化为 `fork_turns="none"` 无法继承上下文。当前 Codex 工具契约的默认是 `fork_turns="all"`，而 `none` 只是显式隔离选项；官方还确认当前 Codex 默认支持 subagent，且可由 AGENTS/skill 请求。这说明理由过时，不说明默认 inline 决策本身错误；主计划应保留默认 inline，避免在没有生命周期证据时改变 dispatch 行为。

候选最小修复：先只改项目自有说明，在 `.trellis/workflow.md` 中补充 Grok/Kimi 的显式手动 inline 步骤，并写明传入 task path/context 的要求；不新增平台别名注册表，不修改 vendor 探测脚本。Codex 保持默认 inline，并把当前理由表述为项目的上下文可靠性策略。若以后维护 Trellis vendor surface，必须同时修正 `.trellis/config.yaml:100-107` 与 `.trellis/scripts/common/workflow_phase.py:144-168` 的同类旧说明；本轮若不改这两处，应在主计划明确延期，不能只改其中一个。检查：文档搜索覆盖五套名称、按 Grok/Kimi 文档各演练一次手动 inline context 构造、`task.py validate`、`just ci`，再分别做五套新会话握手。只有官方证据或实际演练证明手动 fallback 不可行，才另立代码修改方案。

### P1：GitHub Issue 与本地 Trellis PRD 的“唯一入口”表述冲突

`AGENTS.md:74-76` 和 `docs/agents/issue-tracker.md:1-18` 说 issues/PRDs 均在 GitHub Issues；同一个 AGENTS 的 Trellis block 又指定 `.trellis/tasks/` 保存 PRD/research/context（`AGENTS.md:96-101`），workflow 也要求复杂任务写本地 `prd.md`/`design.md`/`implement.md`（`.trellis/workflow.md:147-170`）。应定义两者关系：GitHub Issue 是跨人协作/远端追踪入口，Trellis 是 agent 本地执行契约；只有明确授权才远端创建/评论。检查：文档一致性搜索、issue dry-run/只读查看（不做远端写）、`task.py validate`。

## 五套 harness 能力与本仓库对齐矩阵

| Harness | 官方规则/能力边界（2026-09-07 访问） | 本仓库静态状态 | 判定 |
|---|---|---|---|
| Claude Code | `CLAUDE.md`、`.claude/rules`; skills、hooks、MCP、独立 subagents；permissions + sandbox。明确不自动读 `AGENTS.md`。 | 无根 `CLAUDE.md`；`.claude/` 存在但被忽略，干净克隆缺失。 | **主规则入口未对齐（P1）**；当前本机 hooks/skill 是否启用 `UNVERIFIED`。 |
| Codex | 分层读取 `AGENTS.md`; repo skills 为 `.agents/skills`; `.codex` hooks/MCP；当前默认支持 subagents；本地有 OS sandbox + approval。 | 根 AGENTS 可移植；`.agents`/`.codex` 是忽略的本机镜像/配置；Trellis Codex inline 理由过时。 | **规则入口对齐，扩展契约部分对齐（P1）**；hook trust、skills discovery、subagent profile 均需新会话握手。 |
| Grok Build | xAI 官方 CLI `grok`；读 AGENTS family，也兼容 Claude 配置；原生 `.grok/{skills,hooks,agents,config.toml}`，MCP、subagents、OS sandbox。Grok 对 repo skill/rule discovery 会过滤 gitignored 文件。 | 根 AGENTS 可读；仓库未配置专用 `.grok`，被忽略的 `.claude`/`.agents` 不能作为干净克隆的专用配置证据；Trellis 无 Grok 自动 marker/探测。 | **主规则入口对齐，Trellis 手动路径说明缺失（P1）**；本机版本已探测，专用能力只是静态未配置，实际可用性 `UNVERIFIED`。 |
| Kimi Code | 项目/全局 `AGENTS.md`; `.kimi-code/skills` 与 `.agents/skills`; hooks、MCP、内置 coder/explore/plan subagents，可用 secondary-model 路由便宜模型；文档证明 permission/危险命令 guard，未找到 OS 强制 sandbox 的官方证明。 | 根 AGENTS 可读；当前忽略的 `.agents/skills` 可能供本机发现，但干净克隆没有；仓库未配置专用 `.kimi-code`；Trellis 无 Kimi 自动 marker/探测。 | **主规则入口对齐，Trellis 手动路径说明缺失（P1）**；专用配置缺失不等于原生能力失败，启动发现 `UNVERIFIED`。 |
| Oh My Pi (OMP) | 推荐 `.omp/AGENTS.md`，也通过低优先级 `agents-md` provider 读祖先 `AGENTS.md`; 原生 `.omp` 支持 skills/hooks/MCP/agents；task agents 可绑定 role/model；有 approval policy，未找到内建 OS sandbox 的官方证明。 | 根 AGENTS 可由兼容 provider 读取；仓库未配置专用 `.omp`，Trellis 文本含 Oh My Pi，但当前没有可供静态探测的 `.omp`。 | **主规则有兼容入口**；专用配置只是静态未配置，provider、skills/agent discovery 与自动探测结果均 `UNVERIFIED`。 |

## 模型与任务分工

强模型负责：规则源所有权、Claude/Codex/Grok/Kimi/OMP 的加载优先级与冲突审查；connector/Web 写边界；跨平台路径语义；Trellis dispatch/上下文注入设计；最终 diff 和五客户端握手结果审查。这些任务要求跨文件语义与“静态存在不等于运行时启用”的判断。

便宜模型负责：在已批准且文件清单冻结后，新增 `CLAUDE.md` 薄入口、批量更新明确路径文案、扩展 mirror checker 和表驱动测试、同步 canonical skill 到镜像、运行确定性检查并整理日志。便宜模型不得自行决定权限边界、修改 harness 优先级、安装插件或宣称运行时发现。

建议顺序：先完成 P1 Claude 薄入口与运行时事实修正；再补 Grok/Kimi 的 Trellis 手动 inline 说明并保留 Codex 默认 inline；最后修 P2 mirror gate，并逐 harness 做新会话验证。只有手动 fallback 被官方证据或实测否定时，才设计新的平台探测或 dispatch 代码。每个批准项都应在其 child task 中写明回写目标（`AGENTS.md`、canonical `skills/`、Trellis spec/团队知识库）与适用工具；不要把同一行为复制进五份规则。

## 官方来源

- OpenAI： [AGENTS.md](https://developers.openai.com/codex/guides/agents-md)、[Skills](https://developers.openai.com/codex/skills)、[Subagents](https://developers.openai.com/codex/subagents)、[Hooks](https://learn.chatgpt.com/docs/hooks)、[Agent approvals & security](https://learn.chatgpt.com/docs/agent-approvals-security)（访问 2026-09-07）。
- Anthropic： [Claude Code memory / CLAUDE.md](https://code.claude.com/docs/en/memory)、[Skills](https://code.claude.com/docs/en/skills)、[Subagents](https://code.claude.com/docs/en/subagents)、[Permissions](https://code.claude.com/docs/en/permissions)（访问 2026-09-07）。
- xAI： [Grok Build overview](https://docs.x.ai/build/overview)、[Skills/plugins/hooks/subagents](https://docs.x.ai/build/features/skills-plugins-marketplaces)、[MCP](https://docs.x.ai/build/features/mcp-servers)、[Project rules source](https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/12-project-rules.md)（访问 2026-09-07）。
- Moonshot AI： [Agents and subagents](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/agents)、[Skills](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/skills)、[Hooks](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/hooks)、[MCP](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/mcp)、[Config](https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/config-files)（访问 2026-09-07）。
- OMP： [Context files](https://github.com/can1357/oh-my-pi/blob/main/docs/context-files.md)、[Config discovery](https://github.com/can1357/oh-my-pi/blob/main/docs/config-usage.md)、[Task agent discovery](https://github.com/can1357/oh-my-pi/blob/main/docs/task-agent-discovery.md)、[Settings](https://github.com/can1357/oh-my-pi/blob/main/docs/settings.md)（访问 2026-09-07）。
