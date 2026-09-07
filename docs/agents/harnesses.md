# Agent harnesses

Claude Code, Codex, Grok Build, Kimi Code, and Oh My Pi (OMP) share the repo
contract in [`AGENTS.md`](../../AGENTS.md). Claude Code also loads tracked root
[`CLAUDE.md`](../../CLAUDE.md), which imports `AGENTS.md` with `@AGENTS.md`.

This page records the five-tool matrix. Do not copy five full rule sets.

Evidence date: 2026-09-07. Source research titles from the parent audit that
day: "五套 harness 规则与能力边界审查" and "本地验证记录". Cite those titles
and the date. Do not depend on a live `.trellis/tasks/` path; that directory
moves when the parent task is archived.

## Evidence levels

| Level | Meaning |
| --- | --- |
| Official source | Vendor docs accessed 2026-09-07. |
| Repo static | Tracked files in this clone. |
| PATH version | `--version` on this machine. A version on PATH is not proof that a new session loaded rules. |
| Actual runtime / fresh-start | **UNVERIFIED**. No five-tool fresh-start was authorized. |

Do not mark hosted or fresh-start PASS from documentation work.

## Shared rules

- Project facts live in `AGENTS.md`.
- Claude Code entry is root `CLAUDE.md` (`@AGENTS.md`). `.claude/` is gitignored
  and is not a portable repo contract.
- Codex reads `AGENTS.md`. This repo keeps Trellis Codex `dispatch_mode` default
  **inline** as a project context-reliability choice. Current Codex default is
  subagents with `fork_turns="all"`. `fork_turns="none"` is an explicit isolation
  option. Do not claim Codex sub-agents cannot inherit context because of
  `fork_turns="none"`.
- Grok Build and Kimi Code have no Trellis auto platform marker. Follow the
  manual inline steps below.
- Planning is not approved implementation. Implementation waits for artifact
  review and `task.py start`.

## Five-tool matrix

| Tool | Official rule / skill / permission / delegation (2026-09-07) | Repo static adaptation | PATH version | Actual runtime / fresh-start |
| --- | --- | --- | --- | --- |
| Claude Code | Reads `CLAUDE.md` / `.claude/CLAUDE.md`. Does not auto-read `AGENTS.md`. Skills, hooks, MCP, independent subagents. Permissions plus sandbox. | Tracked root `CLAUDE.md` imports `AGENTS.md`. `.claude/` is gitignored. | 2.1.263 | UNVERIFIED |
| Codex | Layered `AGENTS.md`. Repo skills under `.agents/skills`. `.codex` hooks/MCP. Current default supports subagents (`fork_turns="all"`). Local OS sandbox plus approval. | Root `AGENTS.md` is portable. `.agents/` and `.codex/` are gitignored local mirrors/config. Trellis Codex default remains **inline**. | 0.153.4 | UNVERIFIED |
| Grok Build | Reads the AGENTS family and can use Claude config. Native `.grok/{skills,hooks,agents,config.toml}`, MCP, subagents, OS sandbox. Repo skill/rule discovery filters gitignored files. | Root `AGENTS.md` is readable. No tracked `.grok/` config. Trellis has no Grok auto marker. | 1.0.22 | UNVERIFIED |
| Kimi Code | Project/global `AGENTS.md`. Skills under `.kimi-code/skills` and `.agents/skills`. Hooks, MCP, built-in coder/explore/plan subagents. Permission / dangerous-command guard. No official proof of a forced OS sandbox. | Root `AGENTS.md` is readable. No tracked `.kimi-code/` config. Trellis has no Kimi auto marker. | 0.41.0 | UNVERIFIED |
| Oh My Pi (OMP) | Prefers `.omp/AGENTS.md`. Also reads ancestor `AGENTS.md` through a low-priority `agents-md` provider. Native `.omp` skills/hooks/MCP/agents. Approval policy. No official proof of a built-in OS sandbox. | Root `AGENTS.md` may load through the compatibility provider. No tracked `.omp/` config. Trellis text includes Oh My Pi. | 18.1.12 | UNVERIFIED |

PATH versions come from the 2026-09-07 local validation record on Windows / PowerShell (Claude Code
2.1.263, Codex CLI 0.153.4, Grok 1.0.22, Kimi Code 0.41.0, OMP 18.1.12). A
version on PATH does not prove that a new session loaded rules, skills, hooks,
or subagents.

## Planning versus approved implementation

This distinction applies to all five tools.

1. Task-creation consent is not implementation approval.
2. Planning writes `prd.md`, and for complex tasks `design.md` and `implement.md`.
3. Implementation starts only after review and `task.py start` (status `in_progress`).
4. Do not implement during planning. Do not treat a planning prompt as an approved edit session.

## Grok Build and Kimi Code: manual inline steps

Trellis has no auto platform marker for Grok Build or Kimi Code. Unmatched
`[platform]` blocks in `.trellis/workflow.md` do not inject context.

After `task.py start` and review, implement inline in the main session:

1. Read `.trellis/workflow.md` Phase 2 generic steps.
2. Start the prompt with `Active task: <task path from task.py current>`.
3. Read that task's `prd.md`, `design.md` if present, `implement.md` if present, and relevant specs (trellis-before-dev / spec indexes).
4. After `task.py start` and review, implement inline in the main session. Do not pretend unmatched `[platform]` markers inject context.
5. If session identity is missing, set `TRELLIS_CONTEXT_ID` in the **current process** only. Do not write the user's global environment.

## Session identity

If `task.py current` is empty, set `TRELLIS_CONTEXT_ID` in the current process
only. Do not write user global config. Do not install tools. Do not implement
MCP. Do not publish.

## Official sources (accessed 2026-09-07)

- OpenAI: [AGENTS.md](https://developers.openai.com/codex/guides/agents-md), [Skills](https://developers.openai.com/codex/skills), [Subagents](https://developers.openai.com/codex/subagents), [Hooks](https://learn.chatgpt.com/docs/hooks), [Agent approvals & security](https://learn.chatgpt.com/docs/agent-approvals-security)
- Anthropic: [Claude Code memory / CLAUDE.md](https://code.claude.com/docs/en/memory), [Skills](https://code.claude.com/docs/en/skills), [Subagents](https://code.claude.com/docs/en/subagents), [Permissions](https://code.claude.com/docs/en/permissions)
- xAI: [Grok Build overview](https://docs.x.ai/build/overview), [Skills/plugins/hooks/subagents](https://docs.x.ai/build/features/skills-plugins-marketplaces), [MCP](https://docs.x.ai/build/features/mcp-servers), [Project rules source](https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/12-project-rules.md)
- Moonshot AI: [Agents and subagents](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/agents), [Skills](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/skills), [Hooks](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/hooks), [MCP](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/mcp), [Config](https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/config-files)
- OMP: [Context files](https://github.com/can1357/oh-my-pi/blob/main/docs/context-files.md), [Config discovery](https://github.com/can1357/oh-my-pi/blob/main/docs/config-usage.md), [Task agent discovery](https://github.com/can1357/oh-my-pi/blob/main/docs/task-agent-discovery.md), [Settings](https://github.com/can1357/oh-my-pi/blob/main/docs/settings.md)

## Related files

- [`AGENTS.md`](../../AGENTS.md) — short project facts
- [`CLAUDE.md`](../../CLAUDE.md) — Claude Code import of `AGENTS.md`
- [`docs/agents/issue-tracker.md`](./issue-tracker.md) — local Trellis PRD versus GitHub Issues
- [`docs/agents/release.md`](./release.md) — shared release tag contract
