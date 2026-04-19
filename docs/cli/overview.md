# CLI 总览

## 全局参数

`zot` 支持以下全局参数：

| 参数 | 含义 |
| --- | --- |
| `--json` | 返回标准 JSON envelope，适合脚本和 Agent |
| `--profile <name>` | 选择配置 profile |
| `--library <scope>` | 选择库范围，只支持 `user` 或 `group:<id>` |

## 顶层命令

当前顶层命令来自 `src/zot-cli/src/main.rs`：

- `doctor`
- `config`
- `library`
- `item`
- `collection`
- `workspace`
- `sync`
- `mcp`
- `completions`

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
2. 写操作前确认凭据和 doctor 输出
3. 自动化场景优先加 `--json`
4. 整轮任务只选一种调用路径：`zot ...` 或 `cargo run -q -p zot-cli -- ...`
5. feed 不走全局 `--library` 切换，而是显式用 `library feeds` / `library feed-items`

## 常见起步命令

```bash
zot --json doctor
zot --json config show
zot --json library search "attention" --tag transformer --creator Vaswani --year 2017
zot --json library recent --count 10
zot --json library citekey Smith2024
zot --json library semantic-status
zot --json item get ATTN001
zot --json item merge KEEP001 DUPE001
zot --json item download ATCH005
zot --json item children ATTN001
zot --json collection search Transform
zot --json workspace query llm-safety "What are the main failure modes?" --mode hybrid --limit 5
zot completions powershell
```

## 命令分工

- `config`：查看和修改运行时配置、profile、写凭据
- `library`：默认只读入口；负责搜索、枚举、semantic、feeds、duplicates
- `item`：单条目读取、大多数写操作、附件下载、annotation、Scite
- `collection`：维护真实 Zotero collection，也负责 collection 细粒度读取
- `workspace`：维护本地 reading workspace
- `sync`：检查 preprint 是否已正式发表
- `mcp`：当前只有占位命令，不是可用工作流
- `completions`：生成 bash / zsh / fish / powershell 补全脚本

## 从 ref\zotero-cli 迁移

如果你以前在用 `ref/zotero-cli`：

- `recent 10` 现在对应 `library recent --count 10`
- 通用两项 merge 现在对应 `item merge KEY1 KEY2`
- 旧的 flat top-level alias 和 `--api-base` 不会迁回

完整迁移说明见：[从 ref\zotero-cli 迁移](/guide/migrating-from-ref-zotero-cli)

## 从 ref\zotagent 迁移

如果你以前在用 `ref/zotagent`：

- `sync` 在这里不是附件索引，而是 preprint publication-status sync
- `status` 没有等价单命令，当前要用 `doctor` + `library semantic-status`
- `search-in` / `metadata` / `read` / `expand` 还没补齐
- `s2` 和按 `paperId` 导入也还没补齐

完整对照和补全计划见：[从 ref\zotagent 迁移](/guide/migrating-from-ref-zotagent)

## 子命令导航

- [config](/cli/config)
- [library](/cli/library)
- [item](/cli/item)
- [collection](/cli/collection)
- [workspace](/cli/workspace)
- [sync / mcp](/cli/sync-mcp)
- [故障排查](/cli/troubleshooting)
