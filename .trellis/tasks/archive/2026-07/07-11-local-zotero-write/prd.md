# Zotero 本机安全写入与 skill 路由优化

## Goal

让 `zot` 在没有 Zotero Web API key、无需云端往返时，仍能通过正在运行的
Zotero 桌面客户端安全修改本机库。第一阶段以 duplicate merge/dedupe 为
tracer bullet，同时保留显式 Web API 写入，并让 `zot` skill 能正确选择读取、
desktop 写入和 web 写入路径。

## Background

- `zot-local` 只读访问 `zotero.sqlite`；现有 mutation 通过 `zot-remote` 调用
  Zotero Web API。
- `item merge`、`library duplicates-merge`、`library dedupe --confirm` 当前共享
  Web API 合并引擎，因此 dry-run 或 apply 都可能被 Web 凭据门阻断。
- Zotero Local HTTP API 只支持 GET；桌面 JavaScript API 可在客户端事务内写库。
- 当前实机为 Zotero 9.0.6。其原生 `mergeItems()` 支持跨 item type，并迁移
  children、relations、collections、tags 后将 source 放入回收站；bibliographic
  metadata 仍需在调用前按明确策略填充。
- 归档任务 `07-11-dedupe-cleanup` 和提交 `bb5df55` 已提供 duplicate planner、
  keeper 评分、confidence、字段兼容和 `dc:replaces` 合约；本任务复用这些能力。
- `skills/zot` 是 canonical source；`.agents/skills/zot` 和 `.claude/skills/zot`
  是 `just install` 生成的镜像。
- 直接写 `zotero.sqlite` 会绕过 Zotero 校验、事务与同步语义，本任务禁止该路径。

## Product Decisions

1. 分阶段交付。第一阶段只包含 bridge foundation、merge/dedupe 闭环、
   skill/docs/evals；其他 mutation 命令另立任务。
2. 接受一次性手动安装仓库随附的最小 XPI；不得静默修改 Zotero profile。
3. `zot bridge setup` 生成 XPI 并打开所在目录。用户安装并重启 Zotero 后，
   插件显示五分钟有效、单次使用的配对码；`zot bridge pair <code>` 换取长期 token。
4. 配对成功后，当前配置目标的默认 `write_backend` 设为 `desktop`。没有执行
   setup/pair 的既有 root 或 named profile 因缺省值仍使用 `web`。
5. 全局 `--write-backend desktop|web` 只覆盖当前调用，不改配置。任一后端失败
   后不得自动 fallback 到另一个后端。
6. `library dedupe --confirm` 默认只执行 normal-confidence 组；low-confidence
   组进入 `skipped_low_confidence`。只有 `--include-low-confidence` 或用户明确指定
   keeper/source 的单组合并才允许处理低置信度候选。

## Requirements

- **R1 白名单 desktop bridge**：仓库随附最小 Zotero 9 插件和独立 Rust client；
  协议版本化、JSON DTO 白名单化，不接受脚本或任意字段 mutation。
- **R2 后端选择**：root 和 named profile 均可持久化 `write_backend` 与 desktop
  bridge 凭据；缺省反序列化为 `web`。相关命令输出实际或计划使用的后端。
- **R3 无自动 fallback**：desktop/web 的凭据、错误和作用域独立；调用失败必须
  保留原后端并返回结构化提示。
- **R4 诊断能力**：`doctor` 独立报告 local SQLite read、Local HTTP read、
  desktop bridge write、Web API write 四种能力状态。
- **R5 merge 安全语义**：desktop 后端保留 keeper 类型，只填 keeper 合法且为空
  的 metadata，再在一个 Zotero 事务内调用原生 `mergeItems()`。
- **R6 preview/confirm**：`item merge`、`duplicates-merge`、`dedupe` 继续默认
  dry-run；只有显式 `--confirm` 执行。preview/apply 必须防版本漂移和重放。
- **R7 批量隔离**：dedupe 按组执行；单组事务失败完整回滚，组间失败继续并汇总。
- **R8 scope 正确性**：协议传递 user 或 `group:<public-id>`，由 Zotero 内部解析
  local library ID；跨库、只读库、已删除条目和 child item 都必须拒绝。
- **R9 可审计输出**：结果报告 backend、keeper、sources、metadata fill、跳过字段、
  children、relations、trash、already-applied、失败和 low-confidence skipped 组。
- **R10 secret hygiene**：pairing code、长期 token 不进入日志、fixture、Debug、
  JSON envelope 或文档示例；token 可轮换和撤销。
- **R11 skill 合约**：更新 canonical `zot` skill 的 description、诊断门、后端路由、
  安全门、低置信度规则、失败行为和自然语言示例，不宣称未实现的 desktop mutation。
- **R12 skill 评测**：增加本机 dedupe、显式 web、插件未安装、Zotero 未运行、
  只读近邻和直接 SQLite 写请求的客观 eval，并与修改前 skill 基线比较。
- **R13 兼容性**：保留现有 Web API 行为和标准 JSON envelope；新字段只增不破坏。

## First-Stage Task Map

1. [`07-11-zotero-bridge-foundation`](../07-11-zotero-bridge-foundation/)：无第一阶段
   内部依赖；交付插件、协议、client、setup/pair/status/revoke、配置和 doctor。
2. [`07-11-local-merge-dedupe`](../07-11-local-merge-dedupe/)：依赖 bridge
   foundation；交付三个 merge/dedupe 入口的 desktop writer。
3. [`07-11-local-write-skill-docs`](../07-11-local-write-skill-docs/)：依赖前两项稳定
   的命令和 envelope；更新 skill、eval、镜像验证和双语文档。

父任务只拥有路线图、跨子任务验收和最终集成复核，不直接作为实现目标。审批后
先启动 `07-11-zotero-bridge-foundation`，不要启动父任务。

## Acceptance Criteria

- [ ] **AC1** 无 `ZOT_API_KEY` / `ZOT_LIBRARY_ID` 时，完成 setup/pair 后可通过
  desktop 后端 preview 并确认执行单组合并。
- [ ] **AC2** XPI 在隔离 Zotero 9 profile 可安装、重启、配对、撤销和卸载；
  插件只暴露白名单协议，不存在 eval/execute/script endpoint。
- [ ] **AC3** `item merge` desktop dry-run 不写库；apply 跨类型合并后 source
  进入回收站，附件、notes、tags、collections、relations 保持 Zotero 原生语义。
- [ ] **AC4** `library dedupe` 输出 keeper/absorb/confidence 计划；confirm 默认仅
  normal，low 组进入 `skipped_low_confidence`，显式 include 后才执行。
- [ ] **AC5** Zotero 未运行、未安装、未配对、token 错误、协议不兼容、条目漂移、
  跨库和只读 group 都返回稳定结构化错误，且不触发 web 请求。
- [ ] **AC6** `doctor --json` 分别表达四种能力；token 和 pairing code 始终脱敏。
- [ ] **AC7** 显式 `--write-backend web` 保持现有凭据门、请求和结果兼容；既有未
  setup profile 在升级后仍选择 web。
- [ ] **AC8** CLI/plugin 协议测试覆盖事务回滚、重复请求、响应丢失后的幂等重试、
  invalid Origin/Host/body、版本协商和错误映射。
- [ ] **AC9** skill 路由 eval 能区分 local read、desktop write、explicit web write
  和禁止 SQLite write；canonical 与安装镜像一致。
- [ ] **AC10** `just ci`、XPI 构建检查、docs build、skill eval 和隔离 Zotero smoke
  test 全部通过；真实库演练前先验证可恢复 fixture。

## Out of Scope

- 直接修改 `zotero.sqlite` 或 Zotero profile 数据库。
- Zotero Local HTTP API 写入、任意 JavaScript 执行器、自动 backend fallback。
- 第一阶段接入 note/tag/collection/import/attachment/saved-search/status-sync 等写命令。
- 重写 duplicate detection、keeper 评分或 confidence 算法。
- 永久删除、清空回收站或替用户开启 Zotero 云同步。
