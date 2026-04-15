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

先看 `doctor` 的写权限状态，通常是缺：

- `ZOT_API_KEY`
- `ZOT_LIBRARY_ID`

没有这两个配置时，只能做本地只读分析。

### 2. PDF 提取失败

看 `doctor` 里的 PDF backend 状态。没有可用 backend 时，不要假设能抽取 PDF 文本或批注。

### 3. workspace query 不像语义检索

先检查 embedding 配置：

- `ZOT_EMBEDDING_URL`
- `ZOT_EMBEDDING_KEY`
- `ZOT_EMBEDDING_MODEL`

未配置时会回退到 BM25 或可用模式。

### 4. group 库怎么指定

`--library` 只支持：

- `user`
- `group:<id>`

### 5. MCP 为什么不可用

因为 `zot mcp serve` 目前还没有接入可用实现，当前只能视为保留接口。

## 仍然不对时

建议按顺序检查：

1. 配置文件路径是否正确
2. 环境变量是否存在
3. 当前调用路径是否一致
4. 失败返回里的 `code / message / hint`
