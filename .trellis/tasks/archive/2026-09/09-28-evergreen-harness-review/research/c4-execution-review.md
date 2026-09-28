# C4 实施与事件复核

日期：2026-09-28。适用 Claude Code、Codex、Grok Build、Kimi Code、OMP。用户已批准该子任务，主会话按生效后的 Codex inline 配置实施。C1 安全门先通过；独立 CI/spec 文件与 C3 尾部验证并行，共享 harnesses.md 在 C3 完成后串行编辑。

## 文件与行为

- `.github/workflows/ci.yml`：新增 cron `17 8 * * 1`；stable、msrv、machete、udeps、docs 只在 schedule 跳过；security 复用原有步骤并限制 `contents: read`。
- `docs/agents/harnesses.md`：新增四事件作业矩阵、安全证据时效与真实 scheduled run 的验证边界。
- `.trellis/spec/zot-cli/backend/quality-guidelines.md`：把相同约束回写到现有共享质量契约。

## 事件、依赖、权限与失败传播

| 事件 | 作业 | 展开数量 |
| --- | --- | ---: |
| push main | stable 三平台、msrv、security、machete、udeps、docs | 8 |
| pull_request main | 与原基线相同 | 8 |
| workflow_dispatch | 与原基线相同 | 8 |
| schedule | security | 1 |

人工逐项复核并以 `verify_ci_events.py` 比较基线 YAML：移除批准新增的 schedule、五条事件条件与 security 权限后，所有作业结构、步骤、固定版本和原有事件配置均与基线相同。没有新增 `needs`，schedule 不依赖被跳过作业。没有 job/step 的 `continue-on-error`。security 无条件运行 audit，然后 deny，任一非零退出保持失败传播。权限仅收窄为 `contents: read`，其他作业权限未改变。

## 检查

actionlint 与事件比较均 exit 0。`final-just-ci` 和 `final-docs-build` 均通过。C1 的 audit/deny 结果绑定相同的最终 Cargo.lock，复用原证据。最终 tracked diff 空白检查、任务清单与父子关系检查见 `final-provenance.json` 和 `final-task-validation.json`。

未编辑 deploy-docs.yml、Pages 环境或远端设置。工作流尚未提交到默认分支，实际 scheduled run 保持 UNVERIFIED；本地静态与构建检查不能证明调度已经生效。
