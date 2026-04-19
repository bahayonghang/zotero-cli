# sync / mcp

## sync update-status

`sync update-status` 用于检查 preprint 是否已有正式发表版本。

如果你是从 `ref/zotagent` 迁过来的，要先把这个语义分开：

- `zotagent sync`：附件提取和索引
- `zot sync update-status`：发表状态检查

当前仓库里没有 zotagent 那个 `sync` 的等价命令。

示例：

```bash
zot --json sync update-status ATTN001
zot --json sync update-status --collection COLL001 --limit 20
zot --json sync update-status --apply --limit 20
```

### 什么时候加 `--apply`

- 只想看分析结果：不要加 `--apply`
- 用户明确要求把状态写回 Zotero：再加 `--apply`

`--apply` 会改库，应按写操作标准处理。

## mcp serve

`zot mcp serve` 目前只在命令面上占位，实际会返回未支持状态。

另外，reference MCP 里的 connector 风格 `search` / `fetch` 也不会被搬成独立 CLI 命令；它们在 Rust 版里映射到这些工作流：

- `library search`
- `library citekey`
- `item get`
- `item pdf` / `item fulltext` / `item children`
- `workspace query`

结论：

- 可以在文档里提到 `mcp` 命令存在
- 不要围绕 `mcp serve` 设计实际流程
- 当前可用工作流仍然应该基于 CLI 本身
