# Implement: 本机 merge 与 dedupe 闭环

## Pre-Development

- [x] foundation 已归档/验收，公开 protocol/client/config API 已稳定。
- [x] 本子任务 `task.py start` 后运行 `trellis-before-dev`，读取 zot-cli/zot-core/
  zot-remote backend specs、shared guides 和归档 dedupe spec。
- [x] 运行 `zot --json doctor`，确认 desktop bridge test 状态并保持 installed invocation。

## Stage 1: Model And Writer Abstraction

- [x] `WriteBackend` 进入 Merge/Dedupe result models，JSON lower-case。
- [x] 定义 `MergeWriter` preview/apply 和 production WebMergeWriter；现有 web 逻辑只搬迁。
- [x] backend resolver 和 explicit override/no-fallback tests。
- [x] shared conformance tests 覆盖 empty source、candidate keys、preview/apply output。
- 验证：`cargo test -p zot-cli -p zot-core -- merge`

## Stage 2: Desktop Merge Protocol

- [x] zot-desktop 增加 merge preview/apply DTO 和 client methods。
- [x] plugin 增加 candidate resolution、field/base mapping、fingerprint、plan store、replay。
- [x] plugin 调 Zotero 9 `mergeItems()`，不复制 structural merge。
- [x] fake/protocol tests：plan expiry、drift、cross-library、read-only、invalid child、retry。
- 验证：`cargo test -p zot-desktop`; plugin tests; `just xpi-check`

## Stage 3: CLI Command Wiring

- [x] `item merge` 和 `duplicates-merge` 走 selected writer；human/JSON 显示 backend。
- [x] desktop confirm 单进程 preview+apply，不暴露原 plan token。
- [x] web command regression tests 保持原 request sequence 和 credential errors。
- [x] desktop error fake 断言没有 web writer selection/call。
- 验证：`cargo test -p zot-cli -- item_merge duplicates_merge`

## Stage 4: Dedupe Confidence Gate

- [x] args 增 `--include-low-confidence`。
- [x] `DedupePlan` 标注 planned backend；apply 前 partition normal/low。
- [x] report 增 skipped list/count；默认 low 不调用 fake writer。
- [x] production GroupMerger 使用 selected MergeWriter；逐组串行继续汇总。
- [x] tests 覆盖 normal-only、include-low、single explicit low、one-failure-continues。
- 验证：`cargo test -p zot-cli -- library_dedupe`

## Stage 5: Full Verification

- [x] `just ci`
- [x] `just xpi-check`
- [x] `git diff --check`
- [x] secret scan：真实 token/code 不在 repo、snapshot、日志。

## Zotero 9 Smoke Gate

Waived by the user on 2026-07-11: isolated-profile real-library smoke is not a
completion gate for this child. Windows UI control was unavailable for fixture setup.
The verified development XPI is at
`C:\Users\lyh\AppData\Local\Temp\zotero-cli-local-merge-smoke\zot-bridge-0.6.0.xpi`.
No real merge/dedupe apply was attempted.

### Bridge lifecycle prerequisite discovered during smoke preparation

- [x] Same-profile plugin reload/reinstall keeps authorization and stable instance identity.
- [x] Already-configured `bridge setup` directs to `bridge status`, not a new pairing flow.
- [x] Rust config migrates legacy empty instance identity after successful status without
  changing the selected backend.
- [x] Status, doctor, revoke, and desktop merge reject a different running profile before
  sending the configured bearer token; there is no cross-profile credential copy.
- [x] Focused core/desktop/CLI/plugin regressions and installed `zot --json doctor` pass.
- [x] Real Zotero 9.0.6 XPI upgrade reused the existing authorization without pairing;
  doctor reported status/auth-revoke/merge-preview/merge-apply, and repeated release CLI
  processes returned the same redacted connection ID.
- [x] A legacy config with no instance identity was migrated by successful `bridge status`;
  a subsequent `config show` loaded the same connection ID from disk and retained desktop backend.
- [x] `just install-local` replaced the stale same-version executable; the installed CLI hash now
  matches the verified release binary, and installed `zot` repeats the same doctor/status/config result.

Residual risk accepted by the user: the isolated Zotero profile and recoverable fixture checks
below were not run. The machine currently has only the actively running `default` profile; the
main library was not used for merge preview or apply.

只在隔离 profile 和可恢复 fixture library：

- [ ] cross-type item merge dry-run 后库不变；confirm 后 UI、trash、relations 正确。
- [ ] 验证 in-memory metadata fill 随 native merge 原子保存；故障注入无半写。
- [ ] PDF、annotation PDF、web attachment、普通 attachment、note 行为与 UI merge 一致。
- [ ] read-only group、version drift、plugin shutdown 安全失败。
- [ ] batch normal-only、low skip、include-low、single group recovery。

## Risk And Rollback

- 高风险文件：`commands/item/merge.rs`、`commands/library_dedupe.rs`、core merge models、
  plugin merge module。
- Stage 1 必须先保持 web tests 绿色，再加入 desktop；任何 no-fallback 失败都阻塞。
- 若 Zotero 9 不保证 in-memory fill 与 native merge 同事务，回到规划，不用独立 save 绕过。
- 数据 smoke 只软删除；每轮记录 keeper/source 并验证从回收站恢复。
- 完成本任务后再启动 `07-11-local-write-skill-docs`。
