# 总体设计

## 项目结构与权威
main.rs → clap/output contract → AppContext → commands dispatcher。zot-core 定义配置、模型和 JSON envelope；zot-local 读取 Zotero SQLite 并拥有可写 sidecar；zot-desktop 提供 loopback connector（仅新增导入）；zot-remote 负责 Web API 与外部查询/embedding；zot-cli 负责命令编排。ref 为历史参考，不参与 Rust workspace。
事实源优先 justfile、Cargo.toml、Rust 入口；共享说明 AGENTS.md，操作 skill 源 skills/，镜像为生成状态。

## 设计选择与 AC 机制
- R1/AC1：C1 以共享 AGENTS + 最小 Claude 薄入口 + 单份 harness matrix 校正事实；Grok/Kimi 用明确手动通用 Trellis/inline 路径，不修改平台探测算法。运行时验收单列，不虚构功能等价。
- R2/AC2：统一以日期、SHA、command/run 和 evidence status 记录结果；三个 research 文件保存独立证据；失败当前仍存在、已修复历史、环境缺失分列。
- R3/AC3：C2 复用镜像比较算法扩大默认覆盖；C3 给已有 CI 增加只读 docs build job；C4 修正发布契约而不动正确的 VitePress build。各子任务 design 与 implement 定义具体红/绿验证。
- R4/AC4：每个子任务负责自己的知识回写；README 分入口/验证/发布段，串行整合。C1 建立 docs/agents/harnesses.md，C3 后补验证段。C1 独占 canonical skill 文案，C2 独占检查器。
- R5/AC5：当前只生成 planning 材料；用户批准后才 start 相应子任务。外部 hosted 发布与 fresh-start 证据未到位时分别标 UNVERIFIED。

## 任务依赖
建议执行 C4 → C1 → C2 → C3。C4 技术上独立但优先级高；C1/C2 文件可拆开实施，仍须串行处理 README 和 canonical skill 同步；C3 等待 C1 的共享矩阵和 C2 的门禁稳定。父子关系表达归属，不代替这些依赖。

## 工具与模型
harness 是工具/权限/上下文容器，模型是推理/执行引擎，两者分别选择：
- Claude Code：适合共享规则、skill/hook 和权限语义的强模型审查；便宜模型做已批准薄入口/说明/测试的局部执行。
- Codex：本仓库 Rust/终端反馈和多文件 diff 的强模型主审；较低价执行档做明确脚本和回归，保持当前 inline 约定。
- Grok Build：可作基于官方规则的第二视角审查；本机版本已探测，但低价模型路由未核验，不承诺自动模型分层或隐式加载 ignored skill。
- Kimi Code：plan/explore/coder 和 secondary-model 机制适合规划后交给较低价执行档的受限文档/脚本任务；不得把权限 guard 视为已证实 OS sandbox。
- OMP：可按 task role/model 显式分派强模型审查和便宜模型执行；provider/agent discovery 必须当前会话验证，approval 不等于已证实 OS sandbox。

不列未核验价格，不以品牌代替测试。实际便宜模型以账号当期可用低价档为准，不能决定写入/发布/安全边界。

## 交付和回退
批准后本地修改各自 gate 通过即记录 LOCAL PASS；五工具握手与 release-tag hosted deploy 只有实际证据才 PASS。父任务本地整合可以完成，但完整运行时/部署恢复不能被文档完成替代。
回退仅撤销所属子任务 diff；不迁移或重写历史 tag，不改 global config，不把无关环境缺失纳入修复。
