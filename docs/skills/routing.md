# 路由策略

这一页讲的是：用户在 Claude Code、Codex 里怎么开口时，agent 应该把它理解成什么。

不是讲命令表。

## 按意图路由

| 用户会怎么说 | agent 应该理解成什么 | 返回重点 |
| --- | --- | --- |
| “找我库里 reward hacking 相关的论文” | 先找候选条目 | 哪些条目最相关，为什么 |
| “按 Smith2024 找到那篇论文” | citation key 直达 | 元数据、引用、fallback 说明 |
| “给我看最近 10 条刚进库的文献” | recent-N 枚举 | 最近入库了什么，不是关键词匹配 |
| “把这篇的 PDF 批注和 note 拉出来” | 单篇证据提取 | metadata、children、annotations、缺失能力 |
| “给我建一个 llm-safety workspace” | 建长期主题工作面 | 名称、导入范围、索引前提 |
| “把这个筛选条件存成 saved search” | 保存查询条件 | 保存了什么条件，不是立即结果 |
| “把附件 ATCH005 下载出来” | 下载本地附件 | 附件 key、目标路径、缺失文件 |
| “给这篇文献加一条 note” | 受控写入 | 改了什么、是否具备写权限 |
| “先预览再合并 KEEP001 和 DUPE001” | 手工 merge preview | 会补什么、会移动什么、何时真正写入 |
| “先看当前配置和默认 profile” | 配置排障 | 当前 config、profile、缺什么 |

## 一句话判断

- 单篇或少量条目：优先理解成“查条目”或“取证据”
- 需要长期维护的一组文献：优先理解成 workspace
- 明确说“最近 10 条”“last 10 recent items”：优先理解成 `library recent --count`
- 明确说“保存条件”“以后还要反复用”：优先理解成 saved search
- 明确说“下载附件”“导出文件”：优先理解成附件面
- 明确说“先预览再合并”：优先理解成 `item merge`，不是直接 `duplicates-merge`
- 任何写入：先过安全门
- 任何“为什么不工作”：先过 doctor / config

## 什么时候先跑 doctor

以下场景默认先做环境诊断：

- 第一次接触当前环境
- 任何写操作
- PDF / outline / annotation / attachment 相关任务
- semantic index / semantic search / workspace query
- citation key 查询
- 配置排障
- 用户反馈“为什么不工作”

## skills 页和 CLI 页的分工

- Skills 页回答：“用户怎么开口，agent 会怎么理解”
- CLI 页回答：“底层到底有哪些命令和参数”

先看 skills，后看 CLI。
