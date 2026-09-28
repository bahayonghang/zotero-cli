# C4 / P2：定期安全检查和证据时效

## Goal
依赖在无代码变动时出现新公告，也能被定期扫描发现；保留较低验证成本。

## Evidence
ci.yml:3-8只有push main、PR main、manual。HEAD的8-job成功发生于2026-09-10，rustls公告发布于9月14日，本轮9月28日audit/deny失败。justfile:78-80不含独立安全扫描。

## Requirements
- C4-R1：复用现有security job，增加每周一08:17 UTC schedule；schedule仅运行security。
- C4-R2：push/PR/manual既有job范围与权限不削弱；安全失败必须使job非零，不能continue-on-error或ignore advisory。
- C4-R3：文档说明just ci与安全门的差别，以及SHA相同但公告时间变化时旧PASS失效。

## Acceptance Criteria
- C4-AC1 / R1-R2：事件矩阵复核与actionlint通过；仅schedule跳过stable/msrv/machete/udeps/docs，security保留；原有事件跑完整既有检查。
- C4-AC2 / R2：C1修补后的cargo audit/deny均通过；测试与门禁没有自动改源文件或隐藏非零失败。
- C4-AC3 / R3：harnesses.md/quality-guidelines明确五工具共享安全验收，记录SHA、日期和公告库时效；真实scheduled run未运行则UNVERIFIED。
- C4-AC4：just ci、文档build、git diff --check通过，强模型核对触发和权限无外扩。

## Scope and Dependency
依赖C1；在C3后串行编辑共享说明。写.github/workflows/ci.yml、docs/agents/harnesses.md、.trellis/spec/zot-cli/backend/quality-guidelines.md。不改Pages工作流、不改环境policy、不发起schedule/dispatch/push。2026-09-28 已批准；适用五套工具。
