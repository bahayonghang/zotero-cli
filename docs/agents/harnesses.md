# Agent harnesses

Claude Code, Codex, Grok Build, Kimi Code, and Oh My Pi (OMP) share the project
contract in root `AGENTS.md`. Tracked root `CLAUDE.md` imports that contract with
`@AGENTS.md`. Keep one shared contract and one five-tool matrix.

Evidence date: **2026-09-28**. The audit records are titled "五套 harness 能力与对齐"
and "本地验证记录". Official sources appear below. Use those titles and the date
when locating archived evidence; task directory paths can change after archive.

## Evidence levels

| Level | Meaning |
| --- | --- |
| Official source | Documented capability, reviewed on 2026-09-28. Documentation does not prove local execution. |
| Repo static | Tracked rules, config, workflow, and renderer code in this clone. |
| Local rendering | Tests and `get_context.py` output prove which instructions the renderer selects. They do not start a client or inject context. |
| PATH version | Local `--version` output on Windows / PowerShell. |
| Local discovery | A client inspection result. Only Grok's sanitized `inspect --json` discovery was checked in this audit. |
| Runtime / fresh-start | Five-client fresh sessions, hook execution, model selection, and sandbox enforcement remain **UNVERIFIED**. |

Installed adapters, equal skill mirrors, rendered text, and PATH versions each
have separate proof limits. None proves that a new client session loaded rules,
skills, hooks, or subagents. A permission prompt does not prove OS isolation.

## Shared rules

- Project facts live in `AGENTS.md`. Claude Code uses root `CLAUDE.md` to import them.
- Native directories `.claude/`, `.codex/`, `.grok/`, `.kimi-code/`, `.omp/`, and
  the shared `.agents/` directory are gitignored local installations. Their presence
  on one machine does not establish a clean-clone setup contract.
- `skills/zot` and `skills/zot-brainstorm` are canonical skills. `just skills-sync`
  intentionally generates the two managed mirror roots, `.agents/skills` and
  `.claude/skills`. `just skills-check` checks that contract; it does not certify
  discovery by all five clients.
- **Codex only:** `.trellis/config.yaml` explicitly sets `codex.dispatch_mode: inline`.
  The project override loads implementation and check context in the main session.
  The generated Trellis default is `auto`, which selects `codex-sub-agent` when no
  override is set. The legacy `sub-agent` value also selects that route. The project
  setting does not change another harness or disable Codex's native capabilities.
- All five platforms have matching sections in `.trellis/workflow.md`. Grok Build
  and Kimi Code use pull-based task context requirements in those sections.
- Tool availability, project workflow, user authorization, and model choice are
  separate checks. Documented subagent support alone authorizes no dispatch.
- Use only implemented `zot` commands. `zot mcp serve` still returns
  `mcp-not-implemented`; a harness's MCP support does not add a Zotero MCP backend.

## Five-tool evidence matrix

| Tool / PATH version | Official rules, skills, and delegation | Repo static adaptation | Local discovery / rendering | Unverified behavior |
| --- | --- | --- | --- | --- |
| Claude Code 2.1.283 | Reads `CLAUDE.md`; does not auto-read `AGENTS.md`. Native skills and independent subagents support explicit agent models. Explore inherits the main model. [S3, S4] | Root `CLAUDE.md` contains `@AGENTS.md`. The workflow has `Claude Code` sections. | Local continue command passes `claude`. Alias tests cover meaningful implementation and check bodies. | Fresh session, skill discovery, native hook/context delivery, selected child model, and OS sandbox enforcement. |
| Codex 0.157.1 | Layered `AGENTS.md`; configurable subagent models; delegated permissions follow the documented session boundary. [S1, S2] | Root `AGENTS.md` is tracked. Shared skill mirrors and `.codex/` are local. Explicit project `inline` mode selects main-session work. | Tests cover `auto`, legacy `sub-agent`, explicit `inline`, invalid config, and the current repository config. | Independent fresh session, skill/hook discovery, child model selection, and OS sandbox enforcement. |
| Grok Build 1.0.42 | AGENTS/Claude compatibility, native and compatible skills, inspection, subagents, and configured models. Skill frontmatter `model`/`effort` does not switch the execution model. [S5, S6] | Root `AGENTS.md` is tracked. Workflow `Grok` sections require pull-based task context. No native `.grok/` config is tracked. | Sanitized `grok inspect --json` found gitignored `.agents/skills/zot` and `zot-brainstorm` on this installation. `grok` renders meaningful Phase 2 bodies. | Fresh session, hook/permission effects, dispatch, selected model, and OS sandbox enforcement. Local discovery does not establish behavior for every version or environment. |
| Kimi Code 2.0.0 | Project/global `AGENTS.md`; `.kimi-code/skills` and `.agents/skills` discovery; built-in subagents with isolated contexts and inherited permissions. [S7, S8] | Root `AGENTS.md` is tracked. Workflow `Kimi Code` sections require pull-based task context. No native `.kimi-code/` config is tracked. | Local start skill passes `kimi`. Alias tests cover meaningful implementation and check bodies. | Fresh session, skill discovery, dispatch, independently selected cheaper child model, and OS sandbox enforcement. |
| Oh My Pi (OMP) 18.3.5 | Native `.omp/AGENTS.md` has priority. Ancestor `AGENTS.md` uses the lower-priority `agents-md` compatibility provider, which can be disabled. Task agents support role/model configuration. [S9, S10] | Root `AGENTS.md` is tracked. Workflow `Oh My Pi` sections exist. No native `.omp/` config is tracked. | Local continue command passes `omp`. Alias tests cover meaningful implementation and check bodies. | Fresh session, enabled provider, extension/context delivery, selected model, and OS sandbox enforcement. |

The source review does not establish a forced OS sandbox for Kimi Code or OMP.
No sandbox enforcement test was run for any of the five tools. Read each
session's actual permission and tool policy before work.

| Harness | Permission boundary to inspect | Evidence limit |
| --- | --- | --- |
| Claude Code | Agent tool list and permission mode; project approval still limits the task. | Documented controls do not prove local hook or sandbox enforcement. |
| Codex | Session sandbox/approval policy and delegated permission limits. | Explicit inline config selects a workflow; it does not grant a tool permission. |
| Grok Build | Active permission and sandbox configuration for the selected execution mode. | The discovery inspection did not exercise permission enforcement. |
| Kimi Code | Parent permissions inherited by the built-in subagent. | A command guard does not establish an OS sandbox. |
| OMP | Enabled providers, exposed tools, and the active approval policy. | Provider presence and role files do not prove enforcement or model availability. |

## Native entrypoints and context loading

Use these lowercase IDs with `get_context.py`. Platform matching normalizes
case and separators and maps the three short aliases below. OMP remains
distinct from Pi.

| CLI platform ID | Matching workflow section | Current project Phase 2 route |
| --- | --- | --- |
| `claude` | `Claude Code` | Subagent instructions; native context delivery still needs runtime evidence. |
| `codex` | `codex-inline` through explicit config | Main-session implementation and `trellis-check`. |
| `grok` | `Grok` | Subagent instructions with explicit pull-based context loading. |
| `kimi` | `Kimi Code` | Subagent instructions with explicit pull-based context loading. |
| `omp` | `Oh My Pi` | Subagent instructions; native extension delivery still needs runtime evidence. |

Use the installed native command or skill entrypoint when available. Rendering
is a separate diagnostic operation. For example:

```text
python .trellis/scripts/get_context.py --mode phase --step 2.1 --platform codex
python .trellis/scripts/get_context.py --mode phase --step 2.2 --platform codex
```

Successful rendering must include the action steps. A zero exit status or a
step heading alone is insufficient. The regression test lives at
`scripts/tests/test_harness_platform_routing.py`. After `trellis update`, recheck
that test, the explicit Codex config, and native entrypoint IDs. The alias fix
is a local change to managed `.trellis/scripts/common/workflow_phase.py`;
do not edit `.template-hashes.json` to conceal that change.

For Grok Build and Kimi Code, after approval, artifact review, and task start:

1. Resolve the selected task with `task.py current --source` and render the matching
   Phase 2 step with `--platform grok` or `--platform kimi`.
2. Confirm that the client exposes and authorizes the required native dispatch
   tool. The workflow describes a task role; exposed tools establish how the
   current client can dispatch that role.
3. Start a dispatched prompt with `Active task: <task path>`. Require the child to
   act directly in its assigned role without recursively dispatching the same role.
4. Load `prd.md`, `design.md` and `implement.md` when present, the role's
   `implement.jsonl` or `check.jsonl`, and every referenced spec/research file.
   A matching platform marker does not load these files into a client by itself.
5. If dispatch is unavailable, report the limitation. Use an authorized inline
   fallback only after loading the same task context and preserving the checks.

## Planning and implementation authorization

These rules apply to all five tools.

1. Task creation records scope; implementation requires the user's approval.
2. Complex planning produces reviewed `prd.md`, `design.md`, and `implement.md`.
3. Run `task.py start` after approval and review. Status becomes `in_progress`.
4. Preserve the approved files, write boundaries, and acceptance checks during
   execution. Approval of local changes does not authorize remote publication,
   library mutations, global client configuration, or a paid client trial.
5. If session identity is missing, set `TRELLIS_CONTEXT_ID` in the current process
   only. Do not write the user's global environment.

## Model allocation

Use a strong model for planning, root-cause analysis, security/permission
boundaries, and final review. This allocation is an engineering judgment;
no model price comparison or performance benchmark was performed.

| Work | Planning / review | Execution after scope is fixed |
| --- | --- | --- |
| TLS advisory, dependency graph, and MSRV | Strong model in Codex or Claude Code; inspect the advisory, transitive diff, and compatibility evidence. | A verified lower-cost model can apply a precise lockfile update and run the fixed checks. Return resolution conflicts to the strong model. |
| Platform aliases and context routing | Strong model in Codex or Claude Code; review renderer behavior and each native entrypoint separately. | Implement the reviewed alias map and semantic regression cases. Keep dispatch authorization unchanged. |
| Rules, capability claims, and bilingual docs | Strong model checks the five official sources, rule precedence, and actual local evidence. | Make specified text/link changes and run tests/builds. Do not infer runtime success from text. |
| CI event conditions and permissions | Strong model in Codex or Claude Code reviews the event/job matrix and permission boundary. | Edit the approved YAML, run actionlint, and verify the matrix without a remote dispatch. |

| Harness | Lower-cost execution boundary |
| --- | --- |
| Claude Code | Select an explicit, available agent model and verify the selected model. Explore inherits the main model and does not reduce cost by default. |
| Codex | Use an explicitly selected, available model when authorized. This repo's inline route creates no child model. Native delegation requires applicable authorization and context checks. |
| Grok Build | Verify the configured execution model. Skill frontmatter `model`/`effort` cannot serve as the model-selection mechanism. |
| Kimi Code | Fixed-scope execution is possible; independent lower-cost subagent model selection remains **UNVERIFIED**. Do not promise automatic cost reduction. |
| OMP | Explicit role/model configuration can select an execution model. Verify the active provider and model before use; the audit did not test those settings. |

Every handoff includes the task path, approval status, owned files, applicable
specs, forbidden actions, commands, and expected results. Return security
failures, scope changes, unknown side effects, or missing evidence to the strong
model. Final review retains the strong model.

## Shared verification boundary

Claude Code, Codex, Grok Build, Kimi Code, and OMP share these project checks.

- Local `just ci` runs the Rust, version, Python, and skill-mirror checks. It does
  not run VitePress, audit/deny, or unused-dependency analysis.
- Pull-request CI in `.github/workflows/ci.yml` includes a separate docs job.
  The docs job runs `npm ci` then `npm run build` under `docs/` on Node 20 and
  builds the bilingual VitePress site. The docs job does not deploy GitHub Pages.
- A hosted result on a new SHA remains **UNVERIFIED** until an authorized
  pull-request or push CI run exists for that SHA. Local success does not prove
  a hosted pass.
- GitHub Pages publication remains in `.github/workflows/deploy-docs.yml` on
  published `v*` releases or manual dispatch.

### Security cadence and evidence

The following contract applies to **all five harnesses**. The CI workflow
schedules the existing security job for Monday 08:17 UTC (`17 8 * * 1`).
Only that job runs on the schedule. Its permission is `contents: read`, and
audit/deny failures fail the job. No advisory ignore, failure suppression,
automatic lockfile update, or Pages publication is part of the schedule.

| Event | Required jobs | Expanded job count |
| --- | --- | --- |
| Push to `main` | Stable on Linux/Windows/macOS, MSRV, security, machete, udeps, docs | 8 |
| Pull request targeting `main` | Same complete job set | 8 |
| Manual dispatch | Same complete job set | 8 |
| Weekly schedule | Security only | 1 |

Record the checked commit, UTC time, lockfile hash, tool versions, advisory
database revision, command, and exit code for security evidence. For an
uncommitted patch, keep the baseline commit and changed-file hashes separate.
A security PASS can become stale after a new advisory even when the commit
has not changed. Rerun the security gates for dependency changes and before
delivery; historical CI success alone does not establish current safety.

After editing CI events, run actionlint and review all four event paths, job
dependencies, permissions, and failure propagation. Local validation does not
prove remote scheduling. The new scheduled run remains **UNVERIFIED** until
the workflow reaches the default branch and a hosted run provides evidence
for the checked revision. Detailed gates live in
`.trellis/spec/zot-cli/backend/quality-guidelines.md`.

## Durable writeback

| Scope | Authoritative writeback |
| --- | --- |
| Five-tool shared project facts and safety boundaries | `AGENTS.md` and this page. Keep root `CLAUDE.md` as the `@AGENTS.md` import. |
| Codex-only dispatch policy | `.trellis/config.yaml` and the Codex entries on this page. |
| Shared CLI behavior | Applicable `.trellis/spec/` contracts and bilingual README/docs. |
| Approved operator skill changes | Canonical `skills/` sources, followed by intentional `just skills-sync` and `just skills-check`. Mirror equality alone does not verify a client session. |

Keep project rules out of public Trellis bundled skills and generated native
adapters. New shared skills, team knowledge-base writes, global configuration,
and client trials require authorization for their own scope.

The CLI backend boundary applies to all five tools: local merge/dedupe preview
is read-only; confirmed merge/dedupe mutations use `WebMergeWriter` and require
Web credentials. The Desktop connector only imports new BibTeX/RIS records.
See `src/zot-cli/src/commands/item/merge.rs` and `skills/zot/SKILL.md`.

## Official sources (reviewed 2026-09-28)

- S1: OpenAI [AGENTS.md](https://developers.openai.com/codex/guides/agents-md)
- S2: OpenAI [Subagents](https://developers.openai.com/codex/subagents)
- S3: Anthropic [Claude Code memory / CLAUDE.md](https://code.claude.com/docs/en/memory)
- S4: Anthropic [Subagents](https://code.claude.com/docs/en/subagents)
- S5: xAI [Grok Build overview](https://docs.x.ai/build/overview)
- S6: xAI [Skills/plugins/hooks/subagents](https://docs.x.ai/build/features/skills-plugins-marketplaces)
- S7: Moonshot AI [Agents and subagents](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/agents)
- S8: Moonshot AI [Skills](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/skills)
- S9: OMP [Context files](https://github.com/can1357/oh-my-pi/blob/main/docs/context-files.md)
- S10: OMP [Task agent discovery](https://github.com/can1357/oh-my-pi/blob/main/docs/task-agent-discovery.md)

## Related files

- Root `AGENTS.md`: shared project facts
- Root `CLAUDE.md`: Claude Code import of `AGENTS.md`
- [Issue tracker](./issue-tracker.md): local Trellis acceptance artifacts and GitHub Issues
- [Release contract](./release.md): shared release tag contract
