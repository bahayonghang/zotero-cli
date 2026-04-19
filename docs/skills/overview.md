# Skills 总览

这里的 “skills” 指 `skills/zot-skills/SKILL.md`。

它不是另一套 CLI 教程。它是给 Claude Code、Codex 这类 agent 用的 Zotero 工作流契约。

如果你现在的目标是：

- 在 Zotero 里找条目
- 读取 PDF、批注、note、children
- 建一个长期使用的 workspace
- 保存查询条件
- 下载附件
- 安全改库

那就先走 skill，而不是先背命令。

## 先看这页，再看 CLI

推荐阅读顺序：

1. [Agent 用法](/skills/agent-usage)
2. [路由策略](/skills/routing)
3. [安全边界](/skills/safety)
4. [典型工作流](/skills/workflows)
5. 真要看底层命令，再去 [CLI 总览](/cli/overview)

## 这个 skill 把哪些内容当作一等公民

- 条目元数据：title、creator、year、item type、citation、children
- 证据内容：PDF fulltext、outline、annotations、notes
- 组织结构：tags、collections、libraries、feeds、saved searches
- 主题工作面：workspace、semantic index、semantic query/search
- 配置与排障：doctor、config、profiles
- 受控写入：notes、tags、collections、imports、duplicate merge、publication status sync

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

- 技能正文：`skills/zot-skills/SKILL.md`
- 回归 prompt：`skills/zot-skills/test-prompts.json`
- 量化 eval：`skills/zot-skills/evals/evals.json`
