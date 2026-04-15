# workspace 命令

`workspace` 用来维护主题化 paper set，并支持本地搜索与 RAG-style query。

## 存储约定

- 默认目录：`~/.config/zot/workspaces`
- 工作区文件：`<name>.toml`
- 索引 sidecar：`<name>.idx.sqlite`
- PDF 缓存 sidecar：`.md_cache.sqlite`
- 名称要求：kebab-case，例如 `llm-safety`

## 子命令

```bash
zot --json workspace new llm-safety --description "LLM safety papers"
zot --json workspace list
zot --json workspace show llm-safety
zot --json workspace add llm-safety ATTN001 ATTN002
zot --json workspace remove llm-safety ATTN001
zot --json workspace import llm-safety --collection COLL001
zot --json workspace import llm-safety --tag safety
zot --json workspace import llm-safety --search "reward hacking"
zot --json workspace search llm-safety "alignment"
zot workspace export llm-safety --format markdown
zot workspace export llm-safety --format json
zot workspace export llm-safety --format bibtex
zot --json workspace index llm-safety
zot --json workspace query llm-safety "What are the main causes of reward hacking?" --mode hybrid --limit 5
```

## query 模式

- `bm25`
- `semantic`
- `hybrid`

说明：

- embedding 未配置时，不要假设 `semantic` / `hybrid` 一定有语义结果
- workspace query 与 library semantic-search 复用同一套索引实现

## 推荐工作流

1. `workspace new`
2. `workspace import` 或 `workspace add`
3. `workspace index`
4. `workspace query`

这套流程适合“围绕一个主题长期整理并持续问答检索”的需求。
