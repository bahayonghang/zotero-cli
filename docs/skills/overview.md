# Skills 总览

这里的 “skills” 指 `skills/zot-skills/SKILL.md`。它不是另一个 CLI，而是一份给 AI、Agent 和操作者使用的执行约定。

## 这个 skill 解决什么问题

它帮助执行者快速判断：

- 当前任务该走 `library`、`item`、`collection`、`workspace` 还是 `sync`
- 这是只读任务，还是会改 Zotero 库
- 什么时候先跑 `doctor`
- 什么时候该用 citation key、semantic、feeds、annotations、Scite 这类高级工作流

## 触发范围

这个 skill 只在用户明确要通过 `zot` / Zotero 库 / workspace 做事时触发，例如：

- 在现有 Zotero 库里找条目、导出引用、看 PDF、查批注
- 按 citation key 查文献
- 建 semantic index 或执行 semantic search
- 查看 libraries / feeds / feed items
- 管理 notes、tags、collections、duplicate merge
- 做 annotation 创建、Scite 检查、preprint 状态同步

它**不是**通用的“找论文 / 总结论文” skill。

## 你该怎么用它

1. 把它视为 Zotero 任务的操作手册
2. 先按用户意图决定命令族
3. 新环境、写操作、PDF、semantic、Better BibTeX、异常场景先跑 `doctor`

## 配套文件

- 技能正文：`skills/zot-skills/SKILL.md`
- 回归 prompt：`skills/zot-skills/test-prompts.json`

继续阅读：

- [路由策略](/skills/routing)
- [安全边界](/skills/safety)
- [典型工作流](/skills/workflows)
- [Fallback](/skills/fallback)
