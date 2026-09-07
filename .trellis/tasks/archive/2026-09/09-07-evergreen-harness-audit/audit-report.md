# 常青项目审查与五套 harness 改造计划

审查日：2026-09-07。基线：dev，`5b53e7f5c2dc488160bd2ddf1ba614776403b1d0`。
本轮结论：**没有发现当前 Rust 测试失败或 P0；建议批准 2 个 P1 和 2 个 P2 的本地改造。代码、正式说明和远端状态均未修改。**

## 项目结构与已验证状态

| 层 | 真实入口/关键文件 | 责任与重要边界 |
| --- | --- | --- |
| CLI | src/zot-cli/src/main.rs、context.rs、commands/mod.rs | clap → AppContext → 命令分发；统一 JSON 错误和退出码 |
| core | src/zot-core/src/config.rs、envelope.rs、error.rs | 配置、目录、模型、共享 envelope；配置目录按平台解析 |
| local | src/zot-local/src/db.rs、pdf.rs、semantic.rs、workspace.rs | Zotero 数据源只读；缓存、索引、workspace sidecar 可写 |
| desktop | src/zot-desktop/src/connector.rs | loopback connector；只向当前选中可写目标导入新 BibTeX/RIS |
| remote | src/zot-remote/src/zotero.rs、http.rs、embedding.rs | 其余 Zotero mutation 走 Web API；外部 HTTP/embedding 独立能力 |
| 交付 | justfile、.github/workflows、scripts/、skills/ | 纯 Rust/Python 检查、独立 docs 发布、canonical skill 与本地生成镜像 |

ref/ 不参与 workspace。测试并非只有 inline：另有 JSON error、version guard、search regression、semantic index 集成目标。

| 本轮检查 | 结果 |
| --- | --- |
| just ci | PASS：277 个不同 Rust 测试 + 5 个 Python 测试；fmt/check/clippy 全通过 |
| actionlint | PASS：两个现有工作流 |
| cargo audit / cargo deny | PASS：audit 无已知漏洞；deny 有重复依赖警告但 exit 0 |
| doctor | exit 0；SQLite 可读；connector 不可达，PDF/embedding/Web 写入缺条件 |
| npm --prefix docs run build | BLOCKED：缺 node_modules/VitePress，尚未进入源码构建 |
| 本地 MSRV/nightly/machete/udeps | NOT RUN：工具未安装；未擅自准备新环境 |
| 同 SHA 已有 hosted CI | [7 jobs PASS](https://github.com/bahayonghang/zotero-cli/actions/runs/30210865785)：三平台 stable、Ubuntu MSRV/security/machete/udeps；这是既有运行记录，本轮没有触发新 run |

详细命令、计数、退出码和限制：[local-validation.md](research/local-validation.md)。

## 优先级、根因与批准范围

### P1 / C4：release tag 与 Pages 环境保护策略冲突

release `1.0.0` 的 [run 29644116256](https://github.com/bahayonghang/zotero-cli/actions/runs/29644116256) 构建成功，部署在分配 runner 前被拒绝。当前 github-pages 环境只允许 main branch 与 v* tag；同 SHA 的 main 手动部署成功。根因是 tag 不匹配，不能归因于 VitePress。

**推荐改造**：未来统一 `vX.Y.Z`，Cargo 仍为 `X.Y.Z`；新增 docs/agents/release.md，更新 README.md / README.zh-CN.md 发布入口。保留环境保护和旧标签；不加重复 tag 规则引擎，不改正确的 build。

**必须检查**：人工比对 Cargo 版本、tag 命令草稿与远端 branch/tag policy；actionlint、git diff --check。本地说明完成与真实发布恢复分栏；未来获发布授权后，release tag 的 hosted deploy 成功才证明闭环。

### P1 / C1：项目说明与五工具入口部分不一致

- AGENTS.md:40 的“mutation 全走 Web API”漏掉 connector import-only，与 canonical skill 和 connector.rs 冲突。
- AGENTS.md:45、AGENTS.md:58 把 Linux 路径写成通用路径；Windows doctor 实际使用 Roaming/zot。相关 docs 和 skill 有同类文案。
- 根 CLAUDE.md 缺失；Claude Code 不自动读取 AGENTS.md，需要最小薄入口。
- AGENTS.md:76 的 GitHub PRD 表述未定义与本地 Trellis PRD 的关系。
- Trellis 没有 Grok/Kimi 的原生分支；Codex inline 理由中“fork 不能继承”已过时。缺少可选原生目录不等于该工具不能工作。

**推荐改造**：AGENTS 共享事实 + 根 CLAUDE.md 的 `@AGENTS.md` + 单份 docs/agents/harnesses.md；修正配置路径和测试说明、PRD 权威；给 Grok/Kimi 写手动通用 inline 流程。只改 Codex 相关注释/docstring，不改变默认模式或 vendor 探测算法，不提交个人 hooks/settings。

**文件**：AGENTS.md、CLAUDE.md、docs/agents/harnesses.md、docs/agents/issue-tracker.md、README.md/README.zh-CN.md、中英 agent-usage/getting-started/config/workspace 页面、docs/agents/limits.md、skills/zot/SKILL.md、.trellis/config.yaml 与 workflow_phase.py 的文字。完整路径在 C1 design.md。

**必须检查**：共享入口/事实人工复核；现有 import、desktop、JSON contract 测试；just ci；必要 canonical skill 同步和两个镜像检查；可用客户端新会话的只读加载记录，未运行保持 UNVERIFIED。

### P2 / C2：默认 mirror gate 漏检第二个 skill

安装遍历 skills 下所有目录（justfile:62），默认 checker 却仅指定 zot（scripts/check_skill_mirrors.py:51、scripts/check_skill_mirrors.py:63）。临时目录中故意改坏 zot-brainstorm，默认检查仍 exit 0；显式检查该 skill 则 exit 1。**真实镜像当前一致，缺陷是覆盖不足。**

**推荐改造**：默认枚举 canonical skills，保留现有单树比较和 focused 参数；整组全未装可跳过，部分缺失失败。只改 scripts/check_skill_mirrors.py、scripts/tests/test_check_skill_mirrors.py，必要 justfile 调用与中英 README 验证段。

**必须检查**：第二 skill 漂移、新增 skill、全未装/部分安装、检查不写入、无关 Trellis skill 不误报的回归；修前红、修后绿；Python tests、just ci、diff check。

### P2 / C3：文档编译只在发布时检查

PR CI 没有 docs job；当前 SHA 的 Rust 全绿不能验证后续文档页面变更。当前本地构建因缺依赖受阻，没有证据证明源码坏。

**推荐改造**：.github/workflows/ci.yml 增加独立 docs build，显式 job-level `permissions: { contents: read }`，沿用现有 lock/Node/build；README.md/README.zh-CN.md 与 harness matrix 说明检查边界。保持 Rust just ci 不强制引入 Node，不增加额外 docs recipe。

**必须检查**：批准后 npm --prefix docs ci（仅现有 lock、不升级依赖）、npm --prefix docs run build；临时副本中的确定构建错误返回非零；actionlint、just ci、diff check；未经 remote delivery 授权不触发 hosted run，新增 job 的同 SHA hosted 证据可暂留 UNVERIFIED。

## 历史失败：已修复，避免重复改造

[run 30210314607](https://github.com/bahayonghang/zotero-cli/actions/runs/30210314607) 属于前一个 SHA 4cd1f20：stable 三平台因干净检出没有 .agents/skills/zot 失败；nightly setup 因把日期当成 action ref 失败。当前 5b53e7f 改为全未安装可跳过，以及 `uses: ...@nightly` + `with.toolchain`，同 SHA 随后全部通过。无需重新实现这两项修复。

旧 PR #10 的 pdfium-render 升级运行日志已 HTTP 410，只剩通用失败 annotation；根因 UNVERIFIED，不据此改当前依赖。
完整证据：[workflow-review.md](research/workflow-review.md)。

## 五套 harness：能力、项目对齐与分工

以下推荐是结合本仓库的工程判断，**不是五套模型的性能或价格实测**。模型与 harness 分开选；便宜档以账号当前可用价格为准。

| Harness | 已核验的能力与规则边界 | 当前项目差距 | 推荐承担 |
| --- | --- | --- | --- |
| Claude Code | CLAUDE.md 原生规则；skill/hook/MCP/subagent、permissions/sandbox；不自动读 AGENTS | 缺根薄入口，本机忽略目录不能代表干净克隆 | 强模型审查共享规则、skill 与授权；便宜档做冻结后的文案/测试 |
| Codex | 分层 AGENTS、.agents/skills、subagents 与审批/隔离能力；能力受会话实际工具契约约束 | 根入口存在，inline 注释过时；镜像与实际发现不同 | 强模型负责 Rust/CI 因果和最终 diff；低价档执行脚本/确定回归 |
| Grok Build | AGENTS family、原生 .grok、MCP/subagents；repo discovery 过滤 ignored 内容 | Trellis 原生分支缺失，不能假定 ignored 镜像被发现 | 强模型独立复核工具规则；先用文档化手动流程，不承诺低价自动路由 |
| Kimi Code | AGENTS、.kimi-code/skills 与 .agents/skills；plan/explore/coder、secondary-model、permission guards | Trellis 原生分支缺失；未证实 OS 强制 sandbox | 强模型规划/审查，secondary-model 执行局部文档/Python/YAML |
| OMP | 原生 .omp 与 AGENTS compatibility provider；role/model task agents、skills/hooks/MCP、approval | 根 AGENTS 可通过 provider 读取；未验证 provider/agent 发现，未证实 OS sandbox | 显式强模型 reviewer / 便宜模型 worker；限定文件和完成检查 |

来源（访问 2026-09-07）：[Claude 规则加载](https://code.claude.com/docs/en/memory)、[Codex AGENTS](https://developers.openai.com/codex/guides/agents-md)、[Codex subagents](https://developers.openai.com/codex/subagents)、[Grok Build](https://docs.x.ai/build/overview)、[Kimi agents](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/agents)、[OMP context](https://github.com/can1357/oh-my-pi/blob/main/docs/context-files.md)；逐项扩展能力来源见 [harness-review.md](research/harness-review.md)。

本机 --version 探测均成功：Claude 2.1.263、Codex CLI 0.153.4、Grok 1.0.22、Kimi 0.41.0、OMP 18.1.12。五客户端 fresh-start 都未在本轮运行；不能据此声称全功能兼容或已经对齐，官方当前能力也不能直接外推到每个安装版本。无论由哪套 harness 执行，本项目 `zot mcp serve` 仍是 mcp-not-implemented，公共路径仍为 skill + CLI。

| 问题 | 强模型规划/审查首选 | 可交便宜模型的冻结范围 |
| --- | --- | --- |
| Pages tag/策略根因 | Codex 或 Claude Code；读 GitHub run 与 policy | 准确发布文案和链接，不给发布权限 |
| 规则优先级与工具入口 | Claude Code + Codex 主审；Grok/Kimi/OMP 核自己的原生边界 | 已批准薄入口、路径说明、双语同步 |
| mirror gate | Codex/Claude Code 复核缺失语义 | Kimi secondary-model、OMP worker 或 Codex/Claude 低价档改 Python/test |
| docs CI | Codex/Claude Code 审触发和权限 | 同上，按已有 job 模式加 YAML、跑固定检查 |

## 已记录但不纳入本轮实施包

- Windows/macOS Rust 1.85 尚无 hosted 证据：先在说明标明已验证平台；三平台 MSRV matrix 是可选后续投入，当前没有兼容失败证据。
- Actions/Dependabot major 更新积压：P3 后续单独刷新，不能把旧 PR 红灯当当前失败或一次合并全部升级。
- 旧 Pdfium PR 日志过期、五工具新会话、真实 connector/Web/PDF/embedding 条件不足：保留证据缺口，不编造根因。

## Trellis 与批准动作

已创建父任务和 4 个子任务，全部 planning，均有 prd/design/implement、真实 implement/check JSONL；无模板 seed。

推荐顺序：**C4（发布契约）→ C1（说明/入口）→ C2（mirror）→ C3（docs CI）**。README 和共享矩阵按顺序整合，避免并行重写。每项批准结论回写项目说明/canonical skill，并注明适用工具，不复制到第二份团队知识库。

批准该实施包包含本地修改及列出的验证，C3 包含按现有 lock 准备 npm 依赖；不包含 push、PR、tag/release、Pages policy mutation、客户端安装或用户全局修改。执行细节：[implement.md](implement.md)，验收要求：[prd.md](prd.md)。
