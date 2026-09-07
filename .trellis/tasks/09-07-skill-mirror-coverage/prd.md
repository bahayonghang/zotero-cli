# 补齐 canonical skill 镜像检查覆盖

## Goal

覆盖全部发布 skill 的镜像和干净检出契约，避免漏检；批准后实施

## Requirements

- R1：默认检查覆盖 skills 下全部发布 skill（目前 zot、zot-brainstorm），与 justfile:62 安装范围一致。根因在 scripts/check_skill_mirrors.py:51 与 scripts/check_skill_mirrors.py:63；临时目录复现见父任务 research/local-validation.md。
- R2：保留显式 --canonical/--mirror；整组全未安装可以跳过，任一受管镜像已安装时部分缺失失败。
- R3：复用 compare_trees，检查保持只读；文档区分镜像一致与客户端实际发现。适用于所有调用 just ci 的 harness 及消费镜像的工具。

## Acceptance Criteria

- [x] AC1（R1）：仅第二个 skill 漂移默认入口 exit 1；全部一致 exit 0；新增 canonical skill 自动纳入。
- [x] AC2（R2）：整组全未装且允许跳过 exit 0；任一已装时缺 skill 或另一镜像根 exit 1；显式单 skill 模式有效。
- [x] AC3（R3）本地：检查前后内容不变；现有 missing/extra/content 回归继续通过；just ci 通过并更新验证说明。hosted / 干净检出客户端发现 UNVERIFIED。

## Authority

本轮只完成规划；用户批准后才实施。具体文件、验证机制与顺序见 design.md 和 implement.md；不得以创建本任务推断代码、安装或外部发布授权。
