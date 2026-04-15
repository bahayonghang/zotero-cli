# CLI 总览

## 全局参数

`zot` 支持以下全局参数：

| 参数 | 含义 |
| --- | --- |
| `--json` | 输出标准 JSON envelope，适合脚本与 Agent |
| `--profile <name>` | 选择配置 profile |
| `--library <scope>` | 选择库范围，支持 `user` 或 `group:<id>` |
| `--verbose` | 打开更详细日志 |

## 顶层命令

当前顶层命令来自 `src/zot-cli/src/main.rs`：

- `doctor`
- `library`
- `item`
- `collection`
- `workspace`
- `sync`
- `mcp`

## JSON 输出格式

成功：

```json
{"ok": true, "data": {}, "meta": {}}
```

失败：

```json
{"ok": false, "error": {"code": "...", "message": "...", "hint": "..."}}
```

## 推荐运行习惯

1. 新环境先跑 `doctor`
2. 写操作前先确认凭据
3. 需要自动处理时优先加 `--json`
4. 整轮会话只选一种调用路径：`zot ...` 或 `cargo run -q -p zot-cli -- ...`

## 常见起步命令

```bash
zot --json doctor
zot --json library search "attention"
zot --json item get ATTN001
zot --json workspace new llm-safety --description "LLM safety papers"
zot --json sync update-status --apply --limit 20
```

## 子命令导航

- [library](/cli/library)
- [item](/cli/item)
- [collection](/cli/collection)
- [workspace](/cli/workspace)
- [sync / mcp](/cli/sync-mcp)
- [故障排查](/cli/troubleshooting)
