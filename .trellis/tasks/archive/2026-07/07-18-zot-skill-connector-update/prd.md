# Optimize zot skill for connector-based local access

## Goal

在 CLI 完成 connector 化(两个前置子任务)之后,更新 `skills/zot/` 整套 skill 资产,
使之与新命令面零偏差,并吸收 Codex 官方 `ref/zotero/skills/zotero/SKILL.md` 的结构长处。
**不是 PRD-only**:涉及 SKILL.md、evals.json、test-prompts.json 三份资产 + 镜像重生成,
需要 implement.md 落执行步骤。

## 受影响资产(已核实)

- `skills/zot/SKILL.md` —— frontmatter `description`(路由边界,line 3 含「desktop bridge
  setup/pair/status」)+ 正文(line 15 写入路径、line 145 后端表、line 150 决策顺序、
  安全门、fallback、硬约束多处 bridge)。
- `skills/zot/evals/evals.json` —— 35 条,bridge 契约在 **id 27、28、29、30、31、34、35**
  (例:id 27 断言「Reads capabilities.desktop_write and selected_write_backend」)。
- `skills/zot/test-prompts.json` —— 35 条,schema 为 `{id, prompt, expected}`(**不是**
  trigger_eval.py 的 should_trigger/near_neighbor schema;本仓库无 trigger_eval.py,
  也无自动 eval runner——这两份是 LLM/人工评测 fixture)。
- 镜像:`just skills-check` 按内容哈希校验 `skills/zot` == `.agents/skills/zot` ==
  `.claude/skills/zot`。改完源必须 `just install` 重生成镜像,否则 skills-check 红。

## Requirements

### R1 删除失效内容(SKILL.md)

- frontmatter `description` 去掉「desktop bridge setup/pair/status」,加入「connector 本机
  导入(BibTeX/RIS)」。这是路由边界变更,须做 R4 的触发核对。
- 正文删除 desktop bridge 全部内容:`bridge setup/pair/status/revoke` 路由、bridge 生命
  周期小节、`--write-backend desktop`、`capabilities.desktop_write`、`selected_write_backend`、
  bridge fallback、bridge 硬约束(「Local HTTP 不能替代 bridge 插件」「desktop bridge
  第一阶段只支持 merge/dedupe」等)。
- 写入后端表从三行(local 只读 / desktop bridge / Web API)改为两行:
  - connector 本机导入:仅**新增**条目,目标是 Zotero UI 当前选中 collection,条件是
    Zotero 运行且目标可写;命令 `item import`,写前 `--confirm` 门(**用 --confirm,不用 --yes**)。
  - Zotero Web API:全部 mutation(含 merge/dedupe),条件是 `ZOT_LIBRARY_ID` + `ZOT_API_KEY`。
- 「无 API key 的本机 merge/dedupe」场景改写为:如实告知该能力已随 bridge 移除,
  merge/dedupe 需要 Web API 凭据;不得暗示 connector 能合并。

### R2 新增内容(借鉴 Codex skill)

- 新意图桶「导入文献到 Zotero」:自然语言触发例(「把这个 bib 导入 Zotero」、
  「把这几条 RIS 存进我当前的 collection」),路由到 `item import`,写前复述目标
  collection、可写性与记录数(对应 Codex 的 selected-target + 确认门)。
- 输出契约补两条(借鉴 Codex "Output standards"):
  - 讲清 Zotero item key(如 `PXW99EKT`)与 BibTeX citation key(如 `vaswani_attention_2023`)
    的双 key 区别;
  - 失败时点名具体门:Zotero 未运行 / connector 不可达 / 目标只读 / Web 凭据缺失 /
    无匹配条目 / 写未确认。
- 借鉴 Codex "cite into draft":加一个映射示例,用 `library citekey` + `item cite`/`item export`
  维护 `references.bib` 并在草稿插引用(agent 编辑文件,CLI 供数据;不虚构新命令)。

### R3 evals + test-prompts 同步

- `evals.json`:重写 id 27-31、34-35 —— 删掉 desktop/bridge 断言,改为:
  - 「无 key 想本机去重」→ 期望如实说明需 Web 凭据、不再声称 desktop 能合并;
  - 新增「导入 bib/ris 到当前 collection」eval(期望走 connector import + 复述目标 + 确认门);
  - 新增「双 key 区别」eval。
  - 保持 id 与 test-prompts.json 对齐(两份都 35 条且同 id;若增删须两份同步)。
- `test-prompts.json`:同步改写对应 id 的 prompt/expected;删 bridge 触发样例,
  加 import / 双 key / merge-需要-Web 样例。

### R4 一致性与触发核对

- 安全门清单、能力表、fallback、典型映射逐条对照最终 CLI `--help` 与 `doctor` 输出核对,
  不得出现已删除或未实现的命令(尤其 `item import` 的真实 flag 名以实现为准)。
- 触发核对(无 trigger_eval.py,用人工 + fixture):确认新 `description` 不会误触发
  (near-neighbor:通用找论文/引用格式教学/非 Zotero PDF 处理仍不触发),且 import
  场景能触发;把结论落进 evals/test-prompts 的正例反例里。
- 改完跑 `just install` 重生成镜像,再 `just skills-check` 必须通过。

## Acceptance Criteria

- [ ] SKILL.md 全文(含 frontmatter)grep 无 `bridge`、`--write-backend`、`desktop_write`、`selected_write_backend`、`pair`
- [ ] 写入决策顺序、安全门、fallback、典型映射与新 CLI 实际行为逐条一致(以 `--help` 与 doctor 为准);import 用 `--confirm`
- [ ] 新增「导入文献」意图桶,含 ≥2 自然语言触发例 + 目标可写性/记录数确认门
- [ ] 双 key 区别与「失败点名具体门」两条进入输出契约
- [ ] evals.json / test-prompts.json 均无 bridge 断言,含 import / 双 key / merge-需要-Web 样例,两份 id 对齐
- [ ] `just install` 后 `just skills-check` 通过(三处镜像一致)
- [ ] `just ci` 全绿(skills-check 在内)

## Notes

- 前置:`07-18-connector-local-write` 与 `07-18-remove-bridge-plugin` 均已完成并合入
  (skill 必须描述真实存在的命令面)。
- `skills/zot-brainstorm/SKILL.md` 已核实**无 bridge/write-backend 引用**,本任务不动它;
  执行时再 grep 复核一次。
- 本任务含 implement.md(见同目录);无需 design.md(纯文档改写,无技术架构)。
