# 常青项目与五套 harness 审查报告

审查日期：2026-09-28。基线：dev / ba4e10e987e7afe65dccb8450445d41bc9e08d79，初始工作区干净。

结论：建议批准 **3 个 P1 和 1 个 P2 子任务**。现有 Rust/Python 门全部通过，但独立依赖安全门失败；平台上下文渲染存在 3 个可复现的静默缺失。项目说明已覆盖五工具，但未与当前 Trellis 和本机发现行为完全对齐。本轮只创建规划和证据，没有实施修复。

## 1. 项目结构与阅读范围

| 模块 | 已读关键文件 | 当前责任与边界 |
| --- | --- | --- |
| CLI | main.rs:21-57、context.rs、commands/mod.rs、commands/item/merge.rs:125-150、commands/mcp.rs:7-14 | clap→配置/上下文→分发→JSON/退出码；merge写后端为WebMergeWriter；MCP服务未实现 |
| core | Cargo.toml:1-16、src/zot-core/src/config.rs:184-220、envelope.rs:5-38 | 5-crate workspace；平台配置路径和共享JSON契约 |
| local | src/zot-local/src/db.rs:1733-1751、workspace.rs:30-47、rag_engine.rs:1-8 | Zotero源库只读快照；工作区名称约束；共享索引循环负责sidecar写入 |
| desktop | src/zot-desktop/src/connector.rs:1-5/42-66 | loopback connector仅新增导入；当前选中目标必须可写 |
| remote | src/zot-remote/src/http.rs:31-55、三个reqwest使用crate的Cargo.toml | 公用HTTP/TLS依赖、超时和重定向控制；本轮漏洞来自该依赖链 |
| 验证/交付 | justfile、ci.yml、deploy-docs.yml、deny.toml、check_skill_mirrors.py、脚本测试 | Rust/Python门与独立安全/docs/依赖job分开；发布受环境策略控制 |
| Agent/Trellis | AGENTS.md、CLAUDE.md、harnesses.md、workflow.md、config.yaml、workflow_phase.py、cli_adapter.py、当地五工具适配 | 共享说明与平台规则/运行发现分层；已定位别名与默认策略冲突 |
| Skill | skills/zot/SKILL.md、zot-brainstorm入口与镜像检查 | canonical operator contract；本地镜像不等于所有harness已加载 |

未发现根CONTEXT.md；docs/agents/domain.md明确允许缺失，不把缺失当故障。ref/不参与Rust workspace。审查覆盖结构、关键边界与已发现因果链，不声明已穷尽所有业务缺陷。

## 2. 已运行检查

| 检查 | 当前结果 |
| --- | --- |
| just ci | PASS，exit0；281个不同Rust测试、13个Python测试；fmt/check/clippy/镜像门通过 |
| npm --prefix docs run build | PASS，exit0；使用现有依赖，本机Node26.7.0 |
| actionlint | PASS，exit0 |
| cargo audit | **FAIL，exit1**；rustls0.23.38命中RUSTSEC-2026-0285 |
| cargo deny check | **FAIL，exit1**；同一advisory；其他policy组通过 |
| 五平台get_context实际渲染 | 三个语义失败：claude/kimi/omp仅输出标题，命令仍exit0；codex/grok有正文 |
| grok inspect --json | PASS，仅证明发现；已去敏保存规则/skill/agent来源 |
| zot --json doctor | exit0；SQLite/Pdfium可用；connector不可达、Web/embedding未配置；未做写入或真实PDF/embedding验收 |

Rust统计不重复计算被执行两次的2个version guard。命令日志、时刻、退出码、工具版本见 [本地验证](research/local-validation.md)。本机未安装所需1.85/nightly/machete/udeps，未在本轮本机运行这些门。

同基线SHA已有2026-09-10的8-job hosted成功：三平台stable、MSRV、安全、machete、udeps、docs。该历史记录早于2026-09-14安全公告，不能替代2026-09-28当前扫描。

## 3. 优先级、根因与批准文件

### C1 / P1：锁文件包含新披露的TLS漏洞

Cargo.lock:1558-1570锁定rustls0.23.38；Cargo.toml:39-45为reqwest启用rustls；desktop/local/remote均引入该链。RustSec公告为2026-09-14，CVSS5.3（中等），修补范围>=0.23.45；本机audit/deny均失败。没有实际攻击验证，不能夸大为已证实凭据泄露。

建议：优先精确更新rustls补丁与必要传递依赖，不全量升级、不ignore。文件：**Cargo.lock**；仅解析证明必要时修改**Cargo.toml**。

必须通过：audit、deny、just ci、Rust1.85 locked check、machete、固定nightly udeps、diff检查。兼容性或大版本变化需要重新审查。详见 [C1计划](../09-28-security-lock-refresh/implement.md)。

### C2 / P1：平台别名导致有效步骤正文被静默删除

workflow_phase.py:135-142只移除分隔符后比较。平台ID claude/kimi/omp与工作流显示名Claude Code/Kimi Code/Oh My Pi不等价。已有本机调用位置确实传短ID。探针复现三者各只有45字符标题、exit0；工作流本身存在相应实施段。

建议：在平台匹配边界归一化已证实别名。文件：**.trellis/scripts/common/workflow_phase.py**、新增**scripts/tests/test_harness_platform_routing.py**。保持Codex模式与其他平台行为。

必须通过：修前红/修后绿、五平台2.1和2.2实际CLI有效内容、Codex auto/inline/legacy/非法配置、非目标平台回归、Python discovery、just ci。不能只断言退出码。详见 [C2计划](../09-28-harness-platform-routing/implement.md)。

### C3 / P1：共享说明、当前默认策略和用户能力承诺冲突

- AGENTS.md:90-95、harnesses.md:35-36/79-93说Grok/Kimi无marker，但workflow.md:223/475-526已有支持；Kimi问题是别名失配。
- harnesses.md:30-34/60说项目Codex默认inline；config.yaml:129-145没有启用该键，resolver实际默认auto→codex-sub-agent。
- Grok1.0.42 inspect实际发现gitignored的.agents/skills/zot和zot-brainstorm；旧矩阵的绝对过滤结论不适用于本机结果。
- **README.md:15、README.zh-CN.md:15仍宣称通过Desktop合并**；源码item/merge.rs:125-126只选择WebMergeWriter，canonical skill也限定connector为import-only。

建议：显式Codex inline以落实项目已声明策略；更新五工具矩阵、规则和证据日期；修正双语后端承诺。文件：**AGENTS.md、docs/agents/harnesses.md、.trellis/config.yaml、README.md、README.zh-CN.md**。CLAUDE.md:1的@AGENTS.md已经正确，无需新增五份规则或强制安装五套配置。

必须通过：C2回归、Codex有效inline读回、just ci、VitePress、规则/能力人工交叉核对、diff检查。详见 [C3计划](../09-28-harness-contract-refresh/implement.md)。

### C4 / P2：常青安全验证缺少定期触发

ci.yml:3-8只含push main/PR main/manual；just ci不含安全扫描。旧SHA全绿后出现新公告，现有触发不足以自动刷新该SHA的安全结论。当前安全门逻辑能检出漏洞，无需削弱或替换。

建议：每周一08:17 UTC复用现有security job；schedule只运行安全job，原有事件继续原检查集合。文件：**.github/workflows/ci.yml、docs/agents/harnesses.md、.trellis/spec/zot-cli/backend/quality-guidelines.md**。

必须通过：actionlint，四类事件的job/needs/权限/失败传播矩阵，audit/deny，just ci，VitePress，diff检查。真正scheduled run在远端交付另获授权并运行前UNVERIFIED。详见 [C4计划](../09-28-evergreen-security-cadence/implement.md)。

## 4. 失败工作流的历史因果

| 历史问题 | 当前证据与处置 |
| --- | --- |
| 2026-07-26 clean clone镜像失败、nightly action ref失败 | 原日志已读；当前checker/CI已修，本轮PASS，不重复改造 |
| 2026-07-18 Pages部署失败 | 1.0.0 tag被v*环境策略拒绝；build成功，同SHA/main手动部署成功；release.md已有契约，保留保护和历史标签 |
| 2026-07-08 cargo fmt失败 | 格式差异有日志；当前fmt通过 |
| 旧pdfium-render PR失败 | 日志HTTP410，原因未查明；不据此升级或回退当前PDF依赖 |

运行链接、原始日志和环境策略见 [工作流根因](research/workflow-review.md)。2026-09-07旧审查提出的CLAUDE入口、第二skill覆盖、docs CI已经存在；本轮不重建。

## 5. 五套harness与模型分工

以下为结合仓库证据的工程判断，未做模型质量或费用基准。厂商能力不等于项目默认策略，PATH版本不等于新会话加载成功。

| Harness | 更适合的强模型规划/审查 | 可交低成本执行的范围/限制 |
| --- | --- | --- |
| Claude Code | 共享规则、skill和权限层级；独立复核Rust/CI方案 | 显式低价自定义agent处理固定测试/文案；Explore当前继承主模型，不能默认算低价 |
| Codex | Rust依赖/MSRV、平台函数与CLI因果、最终diff和门禁 | 显式可用低价模型执行冻结的补丁与检查；能力支持不自动授权分派 |
| Grok Build | 通过inspect核对本机发现，审查Grok原生/Claude兼容边界 | 已配置低价model可执行固定范围；SKILL.md的model/effort不能用作模型切换 |
| Kimi Code | 交接内容完整性、独立上下文与内置角色边界 | 适合固定范围执行；本轮未验证独立低价子代理路由，不承诺secondary-model成本效果 |
| OMP | 多provider冲突、role/model选择与执行策略 | 显式低价worker role执行测试/文案；有效provider和模型须核实 |

推荐按问题分配：C1/C2由Codex或Claude Code强模型主审；C3由强模型核对共享契约，再用各工具原生资料/发现结果复核；C4由强模型审事件与权限，低成本模型可做确定的YAML。最终安全和验收结论不降档。

当前版本：Claude2.1.283、Codex0.157.1、Grok1.0.42、Kimi2.0.0、OMP18.3.5。五客户端fresh-start均未执行；Grok额外完成只读inspect。权限审批不等于本机OS sandbox强制保障。本项目zot mcp serve仍未实现。

详见 [能力矩阵与交接规范](research/harness-review.md)及 [11项一手来源](research/sources.md)。

## 6. 批准包与长期写回

已建立主任务和4个子任务，全部planning；每个都包含prd.md、design.md、implement.md及上下文清单。建议顺序 **C1→C2→C3→C4**，可逐项批准。C3依赖C2，C4依赖C1且与C3串行修改共享说明。

批准并验收后，将平台/成本/规则结论写回AGENTS.md和docs/agents/harnesses.md，将安全检查契约写回既有quality-guidelines。每处注明五工具共享或Codex专属。当前canonical Zotero skills已含正确写后端边界，不因开发流程问题新建通用skill；不写团队知识库。

本轮未启动实施任务，未改源码、锁文件、正式说明、CI、canonical skills、全局配置或远端。未运行项保持明确状态，不以文档修复声称五工具端到端兼容。

规划自检通过：5个task.py validate均exit0；15份规划文档、10份上下文清单、父子关系、9处源码锚点及12处本地链接有效；全部planning，无已有tracked diff。结果见research/task-validation.json和research/plan-review.json。该结果是主代理的结构与证据复核，不是实施验收或独立第二审阅。
