# 瘦身 zot 与 zot-brainstorm skill 入口

按下面的顺序实现。先搬运并校对内容，再压缩入口；不要从空白入口开始改写。

## Checklist

1. 以当前 `skills/zot/SKILL.md` 为源，创建 `references/reads.md`、`writes.md`、`doctor.md` 和 `migration.md`。按 `design.md` 的移动表放入原文，先不改写规则。
2. 逐条处理旧的“自然语言到动作的典型映射”。决策已经存在则丢弃重复文本；只有旧桶没有的限制才补进拥有它的参考文件。
3. 重写 `skills/zot/SKILL.md`：使用 `design.md` 的 description，压缩原则、分桶、调用顺序、诊断触发条件、写入内核和输出契约，并给每个参考文件一个命令式相对链接。
4. 对 `skills/zot-brainstorm/SKILL.md` 做同样的先搬运、后压缩。创建 `references/sources.md` 和 `references/report.md`；正文保持英文，description 使用设计稿。
5. 对照旧入口的全部标题做一次覆盖核对。缺任何一条安全、来源或失败规则，就补回对应文件，不要靠后续记忆。
6. 给 `zot` 的 eval 和 test prompt 追加同一条文献综合近邻负例。把 `zot-brainstorm` 两套夹具重排成设计中的 14 个场景；保留各自原 schema。
7. 新增 `scripts/tests/test_skill_entrypoints.py`，断言设计里的入口、description、链接和标题约束。
8. 更新中英文 `docs/skills/overview.md` 的相关文件说明。搜索旧的行号锚点或“完整流程都在根 SKILL.md”的表述；只有找到失真时才改其他页。
9. 运行验证命令。先看静态测试，再生成镜像。镜像漂移只能通过修正 canonical 后重新 `skills-sync` 解决，不能手改 `.agents/skills` 或 `.claude/skills`。

## Validation

内层循环：

```bash
python -m unittest scripts.tests.test_skill_entrypoints
```

完成门：

```bash
just skills-sync
just skills-check
```

`just skills-check` 已经包含 unittest discovery，所以完成门会重跑新测试。未改 Rust 时不必把 `cargo test` 当成内层循环。交付前再跑一次 `just ci`，确认纯检查门仍然通过。

手工核对：

- 旧 `zot` 入口的每个二级或三级标题都在新入口或四个参考文件之一。
- 旧 `zot-brainstorm` 入口的来源解析、证据等级、HTML 约束和 fallback 都还在。
- 两个入口都不超过 250 行，且没有根目录之外的 `SKILL.md`。
- 没有新增或改名 CLI 命令。

## Risks And Rollback

- 最大的风险是压缩时丢掉写入确认、版本条件或跨后端禁止 fallback。第 5 步的标题核对是阻断点；未完成核对不进入镜像同步。
- 不要运行 `just version-sync`。它会重写所有 `skills/**/SKILL.md` 的 description 行。
- 如果静态测试把正文写死，后续只能通过同时修改设计稿和测试来改 description。
- 回滚点是本任务开始前的 `HEAD`。恢复 canonical 文件、新测试和两份 overview 后，重新运行 `just skills-sync`。

## Done

同时满足 `prd.md` 的 AC1–AC7，并在交付里单独写明：没有跑真实 agent 触发实验，路由精度只有静态文本证据。
