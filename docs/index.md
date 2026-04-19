---
layout: home

hero:
  name: zot
  text: "面向 Agent 的 Zotero 文档"
  tagline: "用 zot-skills 查询、提取、整理和安全更新 Zotero 内容的双语指南"
  image:
    src: /images/zot-icon.png
    alt: zot 图标
    width: 220
  actions:
    - theme: brand
      text: "Skills 快速开始"
      link: /skills/overview
    - theme: alt
      text: "典型工作流"
      link: /skills/workflows
    - theme: alt
      text: "CLI 参考"
      link: /cli/overview

features:
  - title: "先看 Zotero 内容面"
    details: "文档先讲 item、PDF、annotation、note、collection、feed、workspace 这些内容面，再落到命令参考。"
  - title: "skill 是主入口"
    details: "`skills/zot-skills/SKILL.md` 是对 Agent 的工作流契约，Rust `zot` CLI 是它背后的执行层。"
  - title: "写入边界明确"
    details: "文档会直接写清楚 doctor 前置、Web API 写权限、安全门，以及 `mcp serve` 当前不可用。"
---

## 从这里开始

- 想知道在 Claude Code / Codex 里怎么自然开口：看 [Agent 用法](/skills/agent-usage)
- 想知道这个 skill 能从 Zotero 里拿什么：看 [Skills 总览](/skills/overview)
- 想看一条完整主线怎么跑：看 [典型工作流](/skills/workflows)
- 如果你以前在用参考 CLI：看 [从 ref\zotero-cli 迁移](/guide/migrating-from-ref-zotero-cli) 和 [从 ref\zotagent 迁移](/guide/migrating-from-ref-zotagent)
- 想直接查命令参考：看 [CLI 总览](/cli/overview)

## 文档范围

本目录优先覆盖三件事：

1. Agent 怎样借助 `zot-skills` 使用 Zotero 里的 metadata、notes、PDF、annotations、collections、feeds 和 workspace
2. 用户在 Claude Code、Codex 等环境里应该怎么用自然语言提出 Zotero 任务
3. Rust `zot` 运行时的前置条件、安全边界和返回约定
4. 需要手动排障或直连运行时时，再去哪一页查 CLI 参考

如果文档和实现不一致，优先相信这些文件：

- `skills/zot-skills/SKILL.md`
- `README.zh-CN.md`
- `README.md`
- `src/zot-cli/src/main.rs`
- `AGENTS.md`
