---
layout: home

hero:
  name: zot
  text: Rust Zotero CLI 文档
  tagline: 覆盖 CLI、workspace 与 zot-skills 的双语使用指南
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
  - title: 对齐当前实现
    details: 命令面以 `src/zot-cli/src/main.rs`、`skills/zot-skills/SKILL.md` 和根 README 为准。
  - title: 覆盖新增能力
    details: 包含 citation key、feeds、semantic index/search、annotations、Scite、duplicate merge 与 attach_mode 工作流。
  - title: 明确边界
    details: 文档会直接写清楚 doctor 前置条件、写操作安全门，以及 `mcp serve` 当前不可用。
---

## 从这里开始

- 第一次接触项目：看 [快速开始](/guide/getting-started)
- 直接执行命令：看 [CLI 总览](/cli/overview)
- 给 AI/Agent 设定操作规则：看 [Skills 总览](/skills/overview)

## 文档范围

本目录主要覆盖两件事：

1. Rust `zot` CLI 的命令、前置条件和能力边界
2. `skills/zot-skills/SKILL.md` 的路由、安全和 fallback 规则

如果文档与实现不一致，优先相信这些文件：

- `src/zot-cli/src/main.rs`
- `README.md`
- `README.zh-CN.md`
- `skills/zot-skills/SKILL.md`
- `AGENTS.md`
