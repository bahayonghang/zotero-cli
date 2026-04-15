# workspace 命令

`workspace` 用来组织主题论文集合，并支持本地检索与 RAG 查询。

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
zot --json workspace index llm-safety
zot --json workspace query llm-safety "What are the main causes of reward hacking?" --mode hybrid --limit 5
```

## query 模式

- `bm25`
- `hybrid`

如果 embedding 未配置，语义查询会退化到可用模式，不需要自己造额外流程。

## 推荐工作流

1. `workspace new`
2. `workspace import` 或 `workspace add`
3. `workspace index`
4. `workspace query`

这套流程适合“围绕一个主题长期整理并问答检索”的需求。
