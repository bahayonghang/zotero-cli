---
layout: home

hero:
  name: zot
  text: Rust Zotero CLI 文档
  tagline: 覆盖 CLI 与 zot-skills 的中英文使用指南
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/getting-started
    - theme: alt
      text: CLI 用法
      link: /cli/overview
    - theme: alt
      text: Skills 用法
      link: /skills/overview

features:
  - title: 基于源码整理
    details: 命令面以 src/zot-cli/src/main.rs 为准，避免文档与实现漂移。
  - title: CLI 全量覆盖
    details: 覆盖 doctor、library、item、collection、workspace、sync 与 mcp 状态说明。
  - title: Skills 工作流
    details: 覆盖 zot-skills 的触发条件、路由规则、安全边界、典型流程与 fallback。
---

## 你可以从这里开始

- 如果你第一次接触这个项目，先看 [快速开始](/guide/getting-started)
- 如果你要直接执行命令，先看 [CLI 总览](/cli/overview)
- 如果你要给 AI/Agent 配置 Zotero 操作规范，先看 [Skills 总览](/skills/overview)

## 文档范围

本目录聚焦两件事：

1. `zot` Rust CLI 的命令与运行习惯
2. `skills/zot-skills/SKILL.md` 的操作约定与工作流

如果命令或能力与文档不一致，优先相信源码与以下文件：

- `src/zot-cli/src/main.rs`
- `README.md`
- `skills/zot-skills/SKILL.md`
- `AGENTS.md`
