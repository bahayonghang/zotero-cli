# 统一项目事实与五套 harness 入口

## Goal

修正说明事实和规则入口，核验五套工具的加载与边界；批准后实施

## Requirements

- R1：修正 AGENTS.md:40 的 Web API 全称表述，纳入 connector import-only；按 src/zot-core/src/config.rs:184 与 src/zot-local/src/workspace.rs:1053 说明跨系统目录；更新 AGENTS.md:69 的集成测试布局。
- R2：消除 AGENTS.md:76、docs/agents/issue-tracker.md 与 .trellis/workflow.md 的 PRD 归属歧义：本地 Trellis PRD 为本任务实施验收源，GitHub issue 为经授权发布的协作记录。
- R3：记录 Claude Code、Codex、Grok Build、Kimi Code、OMP 的规则入口、skill 发现、委派和权限边界；薄入口引用共享事实。
- R4：修正 .trellis/config.yaml 与 .trellis/scripts/common/workflow_phase.py 中将某一 Codex fork 行为泛化的注释/docstring，保持 inline 默认与执行逻辑。为缺少原生 Trellis 路由的 Grok/Kimi 写明确的手动 inline 步骤；五工具都须区分 planning 与已批准实施。
- R5：批准结论回写项目说明/必要 canonical skill，注明工具范围；禁止修改全局配置、安装工具、实现 MCP 或发布。

## Acceptance Criteria

- [x] AC1（R1）：LOCAL PASS — SQLite/Local HTTP read-only、connector import-only、Web API mutations 已写入 AGENTS 与 skill；目录以 `dirs::config_dir()` / doctor 为准并标注 Linux/XDG、macOS、Windows；测试布局含 `src/zot-cli/tests/`。五工具 fresh-start UNVERIFIED。
- [x] AC2（R2）：LOCAL PASS — 本地 Trellis PRD 为实施验收源，GitHub Issues 为经授权协作记录。本轮未执行无授权外部写入。
- [x] AC3（R3、R4）：repo static LOCAL PASS — 五工具矩阵、入口、Grok/Kimi 手动 inline、Codex inline 理由已写入。PATH 版本见 parent `local-validation.md`。Actual runtime / five-tool fresh-start UNVERIFIED。
- [x] AC4（R5）：owner 文件已回写并注明工具。中英用法无相反承诺。Hosted/fresh-start UNVERIFIED。just ci 见实现记录。

## Authority

本轮只完成规划；用户批准后才实施。具体文件、验证机制与顺序见 design.md 和 implement.md；不得以创建本任务推断代码、安装或外部发布授权。
