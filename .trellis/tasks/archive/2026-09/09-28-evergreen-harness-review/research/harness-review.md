# 五套 harness 能力与对齐

2026-09-28；来源编号见sources.md；PATH版本见harness-versions.json。

| Harness/版本 | 官方边界 | 当前项目问题 | 证据等级 |
| --- | --- | --- | --- |
| Claude Code 2.1.283 | CLAUDE.md；独立代理/工具/模型/权限；Explore继承主模型[S3/S4] | 根CLAUDE.md导入正确；continue传claude，工作流Claude Code，正文被过滤 | 文件与渲染已验证；fresh-start/hook/子代理未验 |
| Codex 0.157.1 | 分层AGENTS；模型可配置；分派受用户和会话约束[S1/S2] | 文档inline，config无显式设置，resolver实际auto→codex-sub-agent | 配置/渲染已验；独立fresh-start未验 |
| Grok Build 1.0.42 | AGENTS/Claude兼容、native扩展、inspect、自定义模型；skill model/effort不生效[S5/S6] | 工作流已有Grok，AGENTS却称无marker；inspect实际发现ignored .agents/zot，旧矩阵 blanket ignore说法不能用于本机 | 去敏inspect已验；模型/hook/权限效果未验 |
| Kimi Code 2.0.0 | AGENTS与两套skill路径；内置代理上下文隔离、权限继承[S7/S8] | start传kimi，工作流Kimi Code，正文缺失 | 文件/渲染已验；不沿用未核实的旧secondary-model建议 |
| OMP18.3.5 | .omp优先与可禁用providers；代理role/model配置[S9/S10] | continue传omp，工作流Oh My Pi，正文缺失；AGENTS加载取决于provider | 文件/渲染已验；实际provider/session未验 |

权限提示不能证明本机OS sandbox已强制启用，本轮没有隔离测试。五套工具均只能使用已实现的zot CLI；commands/mcp.rs:7-14仍返回mcp-not-implemented。

## 路由因果
调用位置：.claude/commands/trellis/continue.md:47、.kimi-code/skills/trellis-start/SKILL.md:48、.omp/commands/trellis-continue.md:49。它们是本机gitignored适配，不是干净clone契约。

workflow_phase.py:135-142只去除分隔符后比较，不归一化claude/Claude Code、kimi/Kimi Code、omp/Oh My Pi。filter_platform去除对应内容，get_context保留标题且exit0。workflow.md:475-526实际包含实施段。C2必须测正文语义，不能只测退出码。

## 模型分工建议
| 问题 | 强模型规划/审查 | 冻结后可低成本执行 |
| --- | --- | --- |
| C1 TLS/MSRV | Codex或Claude Code，核对依赖树/公告/兼容性 | 精确更新锁文件、执行检查；冲突交回 |
| C2 上下文路由 | Codex或Claude Code主审，各客户端原生证据验证边界 | 别名和参数化测试，不改授权策略 |
| C3 规则和能力说明 | Claude Code/Codex强模型核对层级，各官方来源验证native行为 | 固定文案、双语同步、链接与构建 |
| C4 CI触发 | Codex或Claude Code审触发/权限/检查范围 | 按事件矩阵改YAML与actionlint，不触发远端 |

Claude显式低价agent、Codex会话可用的低价模型、OMP显式低价role、Grok已配置低价model可作执行通道。Kimi本轮未验证独立低价子代理选择，仅推荐承担固定边界工作，不承诺低价或自动降档。本表是工程判断，没有模型性能/价格排名。

交接包包含任务路径、批准状态、文件归属、spec、禁止事项、命令、预期输出。安全失败、范围扩大、未知副作用或证据缺失交回强模型；最终审查不降档。
