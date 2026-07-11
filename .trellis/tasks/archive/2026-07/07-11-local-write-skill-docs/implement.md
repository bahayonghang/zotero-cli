# Implement: 本机写入 skill 与文档对齐

## Pre-Development

- [ ] foundation 和 merge/dedupe 子任务已完成，记录最终 `--help` 与 JSON fixtures。
- [ ] 本子任务 `task.py start` 后运行 `trellis-before-dev`，读取 docs/spec/shared guides。
- [ ] snapshot `skills/zot` 到 `%TEMP%\zot-skill-workspace\skill-snapshot`。

## Stage 1: Canonical Skill Draft

- [ ] 只编辑 `skills/zot/SKILL.md`、`test-prompts.json`、`evals/evals.json`。
- [ ] 更新 description、capability table、doctor gate、backend decision flow、no fallback。
- [ ] 更新 merge/dedupe safety、low-confidence、setup/pair/status/revoke 和 SQLite refusal。
- [ ] 明确 Phase 1 unsupported desktop mutation，不虚构未来能力。
- [ ] JSON parse/format checks；手工逐条核对命令与 `zot --help`。

## Stage 2: Eval Cases And Assertions

- [ ] 新增至少 8 个设计中的 route cases，同步两份 eval source。
- [ ] assertions 检查 backend、credential gate、preview-confirm、no-fallback、low skip、
  no-SQLite 和 unsupported capability honesty。
- [ ] 创建 iteration-1 eval metadata；运行 new skill 和 old snapshot baseline。
- [ ] 捕获 timing，grade，运行 `scripts.aggregate_benchmark`。
- [ ] 使用 skill-creator `generate_review.py` 生成 viewer 供用户审阅。
- [ ] 根据反馈修订；必要时 iteration-2 重跑并关联 previous workspace。

## Stage 3: Mirror Drift Guard

- [ ] 增加结构化 mirror comparison script 和 tests/fixture。
- [ ] 增加 `just skills-check` 并纳入 `just ci`。
- [ ] 运行 canonical -> mirror generator；不手工编辑镜像。
- 验证：`just _install-skills`; `just skills-check`

## Stage 4: Documentation

- [ ] 从真实 help/JSON fixture 更新 README 和双语 config/library/item/safety/getting-started。
- [ ] 记录 XPI 手动安装、pair/revoke、old profile web default、explicit override、no fallback。
- [ ] 记录 desktop merge 原子性与 web merge 非事务差异。
- [ ] 记录 low-confidence default skip 和恢复/回收站边界。
- 验证：`npm --prefix docs run build`

## Final Validation

- [ ] `just ci`
- [ ] `just skills-check`
- [ ] `npm --prefix docs run build`
- [ ] `git diff --check`
- [ ] canonical/mirror tree hash comparison
- [ ] secret scan（无真实 pair code/token/API key）
- [ ] eval viewer 中关键安全 assertions 100% 通过，用户完成 review。

## Risk And Rollback

- skill source-of-truth 漂移是主要风险；任何 mirror diff 通过 generator 修复。
- eval 基线 snapshot 在临时目录，避免工作树旧文件干扰。
- 如果 docs 与 CLI 冲突，以 clap、JSON fixtures 和已通过 smoke 的实现为准。
- skill 变化可独立 revert，不回滚已稳定的 bridge/merge code。
