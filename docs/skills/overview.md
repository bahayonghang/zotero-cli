# Skills 总览

这里的 “skills” 主要包括：

- `skills/zot/SKILL.md`：Zotero 查询、提取、整理和安全写入的运行时契约
- `skills/zot-brainstorm/SKILL.md`：基于真实 Zotero 文献集合做缺陷分析、brainstorming 和创新点报告

它们不是另一套 CLI 教程。它们是给 Claude Code、Codex 这类 agent 用的 Zotero 工作流契约。

如果你现在的目标是：

- 在 Zotero 里找条目
- 读取 PDF、批注、note、children
- 建一个长期使用的 workspace
- 保存查询条件
- 下载附件
- 安全改库

那就先走 skill，而不是先背命令。

skill 会把本地 SQLite / Local HTTP 只读、desktop bridge merge/dedupe 和显式 Web API mutation 分开路由。desktop 失败不会自动 fallback，未实现的本机 tag/note/collection 写入也不会被虚构。

如果你的目标是：

- 从 Zotero collection、workspace 或显式 item key 里做真实文献驱动的 brainstorming
- 总结研究缺陷、证据边界和下一步创新点
- 默认生成本地 `report.md` 和 `report.html`

那就走 `zot-brainstorm`。

## 先看这页，再看 CLI

推荐阅读顺序：

1. [Agent 用法](/skills/agent-usage)
2. [路由策略](/skills/routing)
3. [安全边界](/skills/safety)
4. [典型工作流](/skills/workflows)
5. 如果你要做文献综述或创新点分析，看 [示例主线](/skills/examples)
6. 如果你以前在用参考 CLI，先看 [从 ref\zotero-cli 迁移](/guide/migrating-from-ref-zotero-cli) 或 [从 ref\zotagent 迁移](/guide/migrating-from-ref-zotagent)
7. 真要看底层命令，再去 [CLI 总览](/cli/overview)

## 这个 skill 把哪些内容当作一等公民

- 条目元数据：title、creator、year、item type、citation、children
- 证据内容：PDF fulltext、outline、annotations、notes
- 组织结构：tags、collections、libraries、feeds、saved searches
- 主题工作面：workspace、semantic index、semantic query/search
- 配置与排障：doctor、config、profiles
- 受控写入：notes、tags、collections、imports、duplicate merge、publication status sync
- 文献综合：reference-grounded brainstorming、缺陷分析、创新点排序、本地 Markdown/HTML 报告

## 在 agent 里怎么理解它

这个 skill 会先回答四件事：

1. 用户要的是哪一类 Zotero 内容
2. 这是只读任务，还是会改 Zotero 库
3. 要不要先跑 `doctor`
4. 最终应该返回结果、证据、边界，还是失败原因

所以在用户视角，正确姿势不是：

- “我该敲哪个命令？”

而是：

- “帮我在 Zotero 里找……”
- “把这篇的批注和 note 拉出来”
- “建一个 workspace，后面我要问答”
- “先看当前配置和 profile”

## 不该触发的场景

默认不走这个 skill：

- 泛化“找论文”
- 普通论文总结
- 引用格式教学
- 不依赖 Zotero / workspace 的 PDF 处理

这些场景没有把 Zotero 当作主要内容源。

## 相关文件

- 技能正文：`skills/zot/SKILL.md`
- 回归 prompt：`skills/zot/test-prompts.json`
- 量化 eval：`skills/zot/evals/evals.json`
- Brainstorm 技能正文：`skills/zot-brainstorm/SKILL.md`
- Brainstorm 报告模板：`skills/zot-brainstorm/templates/report.md` 和 `skills/zot-brainstorm/templates/report.html`
