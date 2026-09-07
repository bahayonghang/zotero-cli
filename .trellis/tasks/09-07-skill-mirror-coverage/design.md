# 设计

默认模式枚举 skills 根下包含 SKILL.md 的直接子目录，针对现有 .agents/skills 与 .claude/skills 两个受管根形成期望组合；不建立 harness 配置注册表。compare_trees 不改，显式单 skill 模式保留。--skip-if-all-missing 在整组受管镜像都不存在时才跳过；部分安装不跳过。无关 Trellis skill 不纳入 extra 判定。

文件：scripts/check_skill_mirrors.py、scripts/tests/test_check_skill_mirrors.py、justfile（仅必要调用调整）、README.md / README.zh-CN.md 的验证段。当前真实镜像均一致，优先级 P2。

AC1 在 TemporaryDirectory 中建立两个 canonical skill，调用真实默认入口并只破坏第二个 skill；修改前必须红。AC2 覆盖全未装、跨 skill 部分安装和显式比较。AC3 比较调用前后文件内容，复用现有单树测试，运行 just ci。只读检查不得自动 skills-sync。

便宜模型负责脚本和回归；Codex/Claude Code 强模型复核 skip 范围、默认和显式模式。回写注明所有 harness 调用共同门禁、两个镜像根的实际消费范围与 fresh-start 限制。回退本任务脚本/测试/文档，不操作安装目录。
