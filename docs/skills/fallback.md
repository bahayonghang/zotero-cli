# Fallback

## 常见兜底策略

### 没有 `zot`

开发环境改用：

```bash
cargo run -q -p zot-cli -- --json doctor
```

### 没有写权限

停在只读分析，并明确提示缺：

- `ZOT_API_KEY`
- `ZOT_LIBRARY_ID`

如果用户已经明确要修配置，下一步先走：

- `zot config show`
- `zot config init`
- `zot config set`

### 没有 Better BibTeX

`library citekey` 只能退回 Extra 字段 fallback，不要假装查过 BBT。

### 没有 PDF backend

不要承诺能提取 PDF 文本、outline 或创建 annotation。

### 没有 embedding

继续使用：

- `workspace query --mode bm25`
- 或 `library search`
- 或让 `hybrid` 自然退化

### `attach-mode auto` 没找到 PDF

条目仍可能成功创建。应该告诉用户“没有找到开放获取 PDF”，而不是把整个命令视为失败。

### 用户给的是父条目 key，不是 attachment key

先指出还缺 attachment key。

必要时先看：

- `item children`
- 再决定是否 `item download`

### `zot mcp serve`

当前不可用，不要围绕它设计流程。
