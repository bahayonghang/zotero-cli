# Fallback

## 常见兜底策略

### 没有 `zot`

改用：

```bash
cargo run -q -p zot-cli -- --json doctor
```

### 没有写权限

停在只读分析，并明确提示缺：

- `ZOT_API_KEY`
- `ZOT_LIBRARY_ID`

### 没有 PDF backend

不要承诺能提取 PDF 文本或批注。

### 没有 embedding

继续使用：

- `workspace query --mode bm25`
- 或让 `hybrid` 自然退化

### `zot mcp serve`

当前不可用，不要围绕它设计流程。

### `item create --pdf` 无法提取 DOI

要求显式提供 `--doi`，不要猜元数据。
