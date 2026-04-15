# 故障排查

## 一条规则：先跑 doctor

```bash
zot --json doctor
```

如果 `zot` 不在 PATH：

```bash
cargo run -q -p zot-cli -- --json doctor
```

## 常见问题

### 1. 不能写 Zotero

先看 `doctor` 的 `write_credentials`。通常缺的是：

- `ZOT_API_KEY`
- `ZOT_LIBRARY_ID`

没有这两个配置时，只能做本地只读分析。

### 2. `library citekey` 找不到结果

先看 `doctor` 里的 `better_bibtex.available`：

- 如果可用，会尝试 Better BibTeX JSON-RPC
- 如果不可用，只能退回 Extra 字段里的 citation key

### 3. PDF / outline / annotation 失败

看 `doctor` 里的：

- `pdf_backend.available`
- `annotation_support.pdf_outline`
- `annotation_support.annotation_creation`

没有可用 backend 时，不要假设能抽取 PDF 文本、outline 或创建批注。

### 4. semantic search 不像语义检索

先检查：

- `embedding.configured`
- `semantic_index`

embedding 未配置时，library/workspace 的检索会退化到 BM25 或可用模式。

### 5. feeds 看不到数据

先看 `doctor` 里的 `libraries.feeds_available`。另外要记住：

- feed 不通过 `--library` 切换
- 应该用 `library feeds`
- 再用 `library feed-items <library-id>`

### 6. `attach-mode auto` 没附上 PDF

这不一定是错误。`auto` 会按 OA cascade 尝试：

1. Unpaywall
2. arXiv relation
3. Semantic Scholar
4. PubMed Central

如果没有开放获取 PDF，条目仍然可能成功创建。

### 7. group 库怎么指定

`--library` 只支持：

- `user`
- `group:<id>`

### 8. MCP 为什么不可用

因为 `zot mcp serve` 目前还没有接入可用实现，当前只能视为保留接口。

## 仍然不对时

建议按顺序检查：

1. 配置文件路径是否正确
2. 环境变量是否存在
3. 当前调用路径是否一致
4. 失败返回里的 `code / message / hint`
