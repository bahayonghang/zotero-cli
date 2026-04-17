# 示例主线

这一页给出一个完整的 Zotero -> ZoteroSynth 文献综述主线示例，目标是让操作者快速理解：

1. 先用 Rust `zot` CLI 做只读检索
2. 再用 ZoteroSynth 的 `review` 工作流做主题综述
3. 明确证据边界，不把 `metadata+abstract` 冒充成 `fulltext`

## 示例任务

用户请求：

> 搜索本地 Zotero 中 `LLM/FoundationModels` collection 下的所有论文，并总结为表格；然后基于这些论文写一篇 “LLM 在时间序列中应用” 的综述文档，并保存为本地 markdown。

## 执行主线

### 1. 先确认执行面与环境

先固定使用 `zot`，并跑一次环境诊断：

```bash
zot --json doctor
```

这个例子里，`doctor` 的关键信息是：

- 本地 Zotero 数据可读
- `pdf_backend.available=false`
- 没有可用的 Zotero Web API 写凭证

因此这是一个**只读**、且**以 `metadata+abstract` 为主证据**的任务。

### 2. 先定位 collection，再取全集

```bash
zot --json collection list
zot --json collection items P5WKAC5Y
```

其中：

- `P5WKAC5Y` 是 `LLM/FoundationModels` 的 collection key
- 共检索到 `114` 篇条目

### 3. 先产出结构化总表

第一步不是直接写综述，而是先把 collection 展平，整理成一个全量表格与统计摘要，方便后续 review 使用。

对应示例文件：

- [collection 总表](./examples/llm-foundationmodels-collection-summary)

这个文件包含：

- 年份分布
- 条目类型分布
- 粗粒度主题分布
- 重复标题检查
- 全部论文的表格清单

### 4. 再走 ZoteroSynth 的 `review` 主线

这个 collection 规模是 `114` 篇，属于 broad review，不应直接按单篇串讲，而应按主题归纳：

- foundation / pretraining
- LLM forecasting adaptation
- multimodal time series
- reasoning / agent
- benchmark / evaluation
- industrial / domain application

同时要遵守 ZoteroSynth 的证据规则：

- 优先级：`fulltext` -> `metadata+abstract` -> `annotations` -> `existing notes`
- 当前示例因为 PDF backend 不可用，所以主证据是 `metadata+abstract`
- 强结论需要标记 `[需确认]`

对应示例文件：

- [综述正文](./examples/review-llm-in-time-series)

### 5. 不伪造 Obsidian 写入

这个示例里没有检测到 `OBSIDIAN_VAULT_PATH`，因此只生成本地 Markdown 预览，不声称已经完成 Obsidian sync。

## 这个示例说明了什么

这个主线体现了两个层级的分工：

- `zot-skills` 负责把 Zotero 检索面跑稳：`doctor`、`collection list`、`collection items`
- `ZoteroSynth` 负责把检索结果变成可复用的分析输出：总表、review、后续可同步的页面

也就是说，真正稳定的做法不是一上来就“让模型自由总结”，而是：

1. 先把语料边界跑清楚
2. 再把证据等级写清楚
3. 最后才进入主题化综述

## 配套示例文件

- [LLM/FoundationModels collection 总表](./examples/llm-foundationmodels-collection-summary)
- [LLM 在时间序列中的应用综述](./examples/review-llm-in-time-series)
