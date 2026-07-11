# 本机写入 skill 与文档对齐

## Goal

在 bridge 与 merge/dedupe CLI 行为稳定后，更新 canonical `zot` skill、路由 eval、
双语文档和安装镜像验证，使 agent 正确区分本地读取、Local HTTP 只读、desktop
本机写入与 Web API 远程写入，不再把所有 mutation 等同于 Web API key。

## Dependencies

- **阻塞依赖**：`07-11-zotero-bridge-foundation` 与 `07-11-local-merge-dedupe` 均已
  完成并通过真实 CLI/envelope 验收。
- 只记录已实现、已测试的命令；若前两项 surface 变化，先更新本任务规划再实施。
- 本任务完成后父任务才能进行最终集成复核。

## Requirements

- **R1 canonical source**：只手工编辑 `skills/zot/*`；`.agents/skills/zot` 和
  `.claude/skills/zot` 由生成流程更新，增加防漂移检查。
- **R2 preserve identity**：skill 名称保持 `zot`；description 扩展 desktop bridge、
  local mutation、dedupe 和 backend routing 触发，不拆 skill。
- **R3 diagnostic gate**：新环境、任何写入、bridge/PDF/workspace 故障仍先运行
  `zot --json doctor`，读取四种 capability，而不是只看 Web credentials。
- **R4 route selection**：本地 read 不启动 bridge；paired/default desktop 的 merge
  使用 desktop；用户明确 remote/web 时使用 `--write-backend web` 并检查 Web credentials。
- **R5 no fallback**：desktop 未安装、Zotero 未运行、auth/protocol 失败时停止并给
  setup/status hint，不得自动改走 web；反向同理。
- **R6 safety gates**：merge/dedupe 先 preview 再 confirm；batch low-confidence 默认
  skipped，skill 不得自行追加 `--include-low-confidence`。
- **R7 scope honesty**：第一阶段只宣称 merge/dedupe desktop write；其他 mutation
  仍按已实现 backend 能力处理，不得把路线图写成现状。
- **R8 SQLite refusal**：直接 SQLite 写请求必须拒绝并解释 local DB read-only 边界。
- **R9 eval coverage**：至少新增 8 个 objective routing eval，覆盖 desktop success、
  explicit web、plugin missing、Zotero stopped、read-only near miss、SQLite write、
  low-confidence 和 unsupported desktop mutation。
- **R10 skill comparison**：修改前 snapshot 作为 baseline，与新 skill 对同一 eval 集
  运行、grade、aggregate，并用 skill-creator viewer 供用户审阅。
- **R11 docs**：更新 README、双语 config/library/getting-started/safety/architecture 或
  相应现存页面，命令、config、错误、安装和恢复步骤与真实 CLI 一致。

## Acceptance Criteria

- [ ] **AC1** “本机无 API key 合并重复”路由 doctor -> bridge status/setup -> dry-run ->
  explicit confirm，不要求 Web key。
- [ ] **AC2** “明确远程写”路由 web，并检查 Web credential；不启动 desktop fallback。
- [ ] **AC3** plugin 未安装、Zotero 未运行和 auth/protocol 失败都停止并给准确 hint。
- [ ] **AC4** local read prompt 不调用 bridge；直接 SQLite write prompt 被拒绝。
- [ ] **AC5** low-confidence batch 不自动 include；unsupported desktop tag/note prompt 不
  虚构支持。
- [ ] **AC6** `evals/evals.json` 和 `test-prompts.json` 同步，新增至少 8 个案例和客观断言。
- [ ] **AC7** old-skill baseline 与 new-skill 结果经过 viewer 人工审阅；关键安全断言
  100% 通过，且无 route regression。
- [ ] **AC8** `just install` 后 canonical、`.agents`、`.claude` 三份文件树逐字节一致，
  `just skills-check` 能在故意漂移 fixture 上失败。
- [ ] **AC9** `npm --prefix docs run build`、`just ci` 和 `git diff --check` 通过。

## Out of Scope

- 为未接入 desktop backend 的 mutation 宣称本机写入支持。
- 重命名、拆分、发布新的 `zot` skill package。
- 在本任务重新设计 CLI protocol 或 merge 行为。
- 将非确定性的 LLM benchmark 放入常规 `just ci`。
