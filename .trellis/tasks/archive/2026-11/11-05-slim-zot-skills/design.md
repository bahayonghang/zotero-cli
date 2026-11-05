# 瘦身 zot 与 zot-brainstorm skill 入口

## Boundaries

只改 canonical skill 包、直接失真的仓库引用，以及一个由现有 `skills-check` 运行的静态测试。

- 可改：`skills/zot/**`、`skills/zot-brainstorm/**`、`scripts/tests/test_skill_entrypoints.py`、`docs/skills/overview.md`、`docs/en/skills/overview.md`。
- 只在链接或“完整契约都在根 SKILL.md”这类表述失真时，才改其他文档。
- 不改：Rust 源码、`justfile` 的 `version-sync`、`scripts/check_skill_mirrors.py`、镜像目录、`.trellis/spec/`。
- Codex 当前是 `dispatch_mode: inline`。本任务不策展 `implement.jsonl` 或 `check.jsonl`；保留现有 seed 行，不把它当成已策展上下文。

## Description Contract

两条 description 都使用下面的确切单行正文，再加现有后缀 ` (v1.1.0)`。不要改成 `description: |`。`version-sync` 会去掉旧版本后缀并写入 `Cargo.toml` 的当前版本；实现时不要运行 `version-sync`，避免它重写手工文本之外的格式。

`zot`：

```text
当用户要直接查询、提取、整理或安全更新本机已有的 Zotero 库或 reading workspace 时使用，即使用户没说 zot。覆盖库内搜索、citation key、PDF/批注/笔记、collection、saved search、feed、附件、语义索引、Scite、配置排障、Zotero 10+ Local API 字段/作者/分类/单条标签、connector 导入 BibTeX/RIS，以及明确授权的 Web API 写入。不要用于泛化找论文、引用格式教学、非 Zotero PDF，也不要用于文献缺陷分析、创新点或 brainstorm 报告；后者使用 zot-brainstorm。
```

`zot-brainstorm`：

```text
当用户要基于已有 Zotero collection、workspace 或明确 item key，做研究 brainstorming、缺陷/局限/gap analysis、创新方向或下一步选题，并生成可追溯的 Markdown/HTML 报告时使用，即使用户没说 zot-brainstorm。不要用于普通库内检索、单篇摘要、标签/笔记/分类维护、引用格式教学、脱离 Zotero 的网页找论文，或未经单独授权写入 Zotero/Obsidian；这些分别交给 zot 或停止。
```

这两条比现状更短，但把姊妹 skill 、中文触发词和排除项放进同一条不会被截断的 description。`zot` 正文仍用中文，`zot-brainstorm` 正文仍用英文；本任务不做正文翻译。

## Package Layout

```text
skills/zot/
  SKILL.md
  references/reads.md
  references/writes.md
  references/doctor.md
  references/migration.md
  evals/evals.json
  test-prompts.json
skills/zot-brainstorm/
  SKILL.md
  references/sources.md
  references/report.md
  templates/report.md
  templates/report.html
  agents/openai.yaml
  evals/evals.json
  test-prompts.json
```

不单独建示例集。现有“自然语言到动作的典型映射”与意图桶大量重复；实现时逐条归入拥有该决策的参考文件，不整节搬运。

### `zot` 移动表

| 旧入口内容 | 去向 |
| --- | --- |
| 原则、调用顺序、输出契约 | 压缩后留在根入口 |
| 十个意图桶的一行路由和停止条件 | 留在根入口 |
| 1–5、9，以及 10 的 `item versions` / `item deleted` | `references/reads.md` |
| 6、7、7a–7d，以及 `sync update-status` | `references/writes.md` |
| 安全门的命令清单、三层分类和确认参数 | `references/writes.md`；根入口只留停止规则 |
| 诊断字段清单、硬约束、语义差异、失败 fallback、配置排障 | `references/doctor.md` |
| 两份迁移表及“不要假装命令存在” | `references/migration.md` |

根入口的写入内核必须保留这些可执行判断：只读请求不写库；预览不等于已写入；确认参数必须来自同一次预览；low-confidence 默认不执行；失败不改走另一条后端；SQLite 永远不写；connector 只能新增导入。

诊断内核只留“何时跑 doctor”以及四个改变路由的能力：`desktop_read`、`local_api_write`、`connector_write`、`web_write`。其余字段进 `references/doctor.md`。

### `zot-brainstorm` 移动表

| 旧入口内容 | 去向 |
| --- | --- |
| 触发、排除、只读规则、分析顺序、最终回复契约 | 留在根入口 |
| collection、workspace、item key 的命令解析和证据等级 | `references/sources.md` |
| 报告章节、默认路径、HTML 约束、来源失败与降级 | `references/report.md` |
| 报告正文模板 | 继续只放在 `templates/`，不把模板拷回入口 |

根入口仍要写明：缺失请求的来源就停止；歧义候选先问用户；只有 metadata/abstract 时不得写成全文结论；默认同时生成 Markdown 和 HTML。

## Root Shape

每个意图桶在根入口最多四行：用户语言、首选命令、一条停止条件、一个相对链接。链接使用普通相对 Markdown 链接，例如 `references/writes.md`，不使用行号锚点。

链接旁边要有命令式，不能只写“详见参考”。例如：“字段、作者、分类成员或单条标签要写入时，先读 references/writes.md，按其中的预览和版本条件执行。”

目标体积是每个入口 250 行以内，实际应落在 140–220 行。参考文件没有单文件行数上限，但不得把两份旧入口原样拼接到一个文件后再留一份副本。

## Regression Contract

新增 `scripts/tests/test_skill_entrypoints.py`，并由现有 `python -m unittest discover -s scripts/tests -p "test_*.py"` 执行。测试只做静态检查：

- `skills` 下只有 `skills/zot/SKILL.md` 和 `skills/zot-brainstorm/SKILL.md` 两个精确名为 `SKILL.md` 的文件。
- 每个入口不超过 250 行，且只有一行匹配 `^description: .+ \(v<cargo-version>\)$`。
- 两条 description 分别包含上面确切正文。
- 入口中的相对 Markdown 链接都解析到该 skill 目录内的非空文件，且不指向目录外。
- `zot` 入口仍包含调用顺序、诊断、安全和输出契约四个标题。
- `zot-brainstorm` 入口仍包含只读、证据等级、分析顺序和报告输出四个概念。

`zot` 的原 46 条保持原顺序和原期望路由。在两份文件末尾同时追加一条近邻负例：要求基于已有 collection 做缺陷分析、创新点和本地报告。期望是交给 `zot-brainstorm`，不把它展开成普通库搜索或写入。

`zot-brainstorm` 的两份夹具改成同一顺序的 14 个场景：

1. 两个 collection 的默认双格式报告。
2. 歧义 collection 名必须先让用户选择。
3. 只有 metadata 和 abstract 时限制结论强度。
4. workspace 及必要时的 query / BM25 降级。
5. 显式 item key；缺失 key 不能悄悄丢弃。
6. collection、workspace 和 item key 混合输入按 item key 去重。
7. 大语料先给覆盖范围，不逐篇展开。
8. 未指定路径时使用默认双文件路径。
9. 英文请求生成英文报告。
10. 中文请求只使用 Zotero 来源。
11. 拒绝未单独授权的 Zotero note 和 Obsidian 写入。
12. 单篇摘要不触发本 skill。
13. HTML 报告必须离线自包含。
14. 不发明 `zot brainstorm` 或其他不存在的命令。

两份文件保持原有 schema：eval 使用 `expected_output` 和 `expectations`，test prompt 使用 `expected`。只对齐场景顺序和覆盖，不把两种格式合并成第三套。

对依赖细节文件的样例，在期望里加上应读的相对路径。普通路由样例不强制加路径，避免把夹具变成文件名清单。

## Docs

两份 overview 的“相关文件”增加对应 `references/` 目录，并说明根 `SKILL.md` 是路由入口，细节在同包参考文件。不改其他教程页的命令示例。

## Compatibility And Rollback

新增文件会被现有整树镜像复制和比较覆盖，所以不改镜像脚本。回滚时恢复本任务改动的 canonical 文件、测试和两份 overview，再运行一次 `just skills-sync`；不要手工删除镜像里的旧文件。

本次没有数据迁移。静态测试不能证明真实 agent 会按新 description 触发；这一点在交付时保持为 missing evidence。

## Trade-offs

- 保留单行 description，是为了不改已有版本同步器。代价是文本不能按语义换行，所以正文必须本身就短。
- 用四个 `zot` 参考文件而不是一个大文件，让读任务和写任务不必同时加载迁移表和写入细节。
- 不把映射示例单独成文，避免第三份路由说明继续漂移。代价是实现时要做一次逐条归并，不能只做文件搬运。
