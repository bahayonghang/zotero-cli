# sync / mcp

## sync update-status

`sync update-status` 用于检查 preprint 是否已有正式发表版本。

示例：

```bash
zot --json sync update-status ATTN001
zot --json sync update-status --collection COLL001 --limit 20
zot --json sync update-status --apply --limit 20
```

### 什么时候加 `--apply`

- 只想看分析结果：不要加 `--apply`
- 用户明确要求把状态写回 Zotero：再加 `--apply`

`--apply` 属于会改库的动作，应按写操作标准处理。

## mcp serve

`zot mcp serve` 目前只在命令面上占位，当前实现返回未支持状态。

结论：

- 可以在文档里提到它存在
- 不要围绕它设计实际流程
- 当前可用工作流仍然应该基于 CLI 本身
