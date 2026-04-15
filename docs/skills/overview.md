# Skills 总览

这里的 “skills” 指仓库中的 `skills/zot-skills/SKILL.md`，它不是另一个 CLI，而是一份给 AI / Agent / 操作者使用的执行约定。

## 这个 skill 解决什么问题

它帮助执行者快速判断：

- 这是单次文献查询，还是持续使用的 workspace
- 这是本地只读任务，还是需要走 Zotero Web API 的写操作
- 什么时候先 `doctor`
- 什么时候必须做安全确认

## 触发范围

根据 `SKILL.md`，以下场景都适合使用该 skill：

- Zotero / 文献库 / papers / references / citations / bibliography
- PDF attachments / collections / tags / notes
- reading workspaces / paper RAG
- “找论文” / “导出引用” / “整理 Zotero” / “做阅读工作区” / “查 PDF 内容” / “同步 preprint 状态”

## 你该怎么用它

1. 把它视为 Zotero 任务的操作手册
2. 先按用户意图判断应该走 `library` / `item` / `collection` / `workspace` / `sync`
3. 任何新环境、写操作、PDF、workspace 问题都先看 `doctor`

## 配套文件

- 技能正文：`skills/zot-skills/SKILL.md`
- 回归 prompt：`skills/zot-skills/test-prompts.json`

继续阅读：

- [路由策略](/skills/routing)
- [安全边界](/skills/safety)
- [典型工作流](/skills/workflows)
