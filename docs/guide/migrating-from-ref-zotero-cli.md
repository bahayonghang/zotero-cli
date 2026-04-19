# 从 `ref/zotero-cli` 迁移

这页只回答一件事：如果你以前用的是 `ref/zotero-cli`，现在在 Rust 版 `zot` 里该怎么理解和替换。

## 先看结论

| 类型 | 结论 |
| --- | --- |
| 已等价覆盖 | `search`、`get`、`annotations`、`notes`、`collections`、`collection`、`add doi/url`、`tags` |
| 这轮新增补齐 | `recent [n]`、显式 `item merge`、`completions <shell>` |
| 明确不迁回 | `--api-base`、flat top-level alias、compact JSON 默认输出、connector 风格主命令 |

## 命令映射

| `ref/zotero-cli` | 当前 `zot` |
| --- | --- |
| `search <query>` | `library search <query>` |
| `get <key>` | `item get <key>` |
| `annotations <key>` | `item annotation list --item-key <key>` 或 `item pdf <key> --annotations` |
| `notes <key>` | `item note list <key>` |
| `collections` | `collection list` |
| `collection <id>` | `collection items <id>` |
| `add doi <doi>` | `item add-doi <doi>` |
| `add url <url>` | `item add-url <url>` |
| `tags` | `library tags` |
| `recent 10` | `library recent --count 10` |
| `merge KEY1 KEY2` | `item merge KEY1 KEY2` |
| `completions powershell` | `completions powershell` |

## 这轮新增的补齐点

### 最近 N 条

旧用法：

```bash
zotero-cli recent 10
```

新用法：

```bash
zot --json library recent --count 10
```

如果你要的是“某天之后的最近条目”，现在还可以继续用时间边界模式：

```bash
zot --json library recent 2026-04-01 --limit 20
```

这两个语义分开了：

- `--count` 是最近 N 条
- `<since> --limit` 是按时间边界筛

### 手工 merge

旧参考实现里可以直接合并任意两条。现在也有了显式命令：

```bash
zot --json item merge KEEP001 DUPE001
zot --json item merge KEEP001 DUPE001 --confirm
zot --json item merge KEEP001 DUPE001 --keep DUPE001 --confirm
```

默认先 preview，不加 `--confirm` 不落库。

preview 会返回：

- 会补齐哪些 metadata 字段
- 会新增哪些 tags / collections
- 会 re-parent 多少 children
- 会跳过多少重复 attachment

如果你手里本来就是一组重复候选，仍然可以先走：

```bash
zot --json library duplicates --method both
zot --json library duplicates-merge --keeper KEEP001 --duplicate DUPE001 --confirm
```

`duplicates-merge` 和 `item merge` 现在复用同一套合并规则。

### completions

现在可以直接生成 shell completion：

```bash
zot completions bash
zot completions zsh
zot completions fish
zot completions powershell
```

## 为什么不把旧命令原样搬回来

Rust 版不是 connector 的平铺包装层。它的边界已经写死了：

- 本地读取来自 `zotero.sqlite` 和附件目录
- 写操作只走 Zotero Web API
- 输出优先稳定 JSON envelope
- 主心智是 `library` / `item` / `collection` / `workspace` / `sync`

所以这几类旧特性不会补：

- `--api-base`
- `search` / `get` 这一类 flat top-level alias
- compact JSON 作为默认输出
- 把 connector 风格 `search` / `fetch` 再做成第二套主命令

## 对 agent / skill 的影响

如果你装了 `zot-skills`，迁移后的自然语言路由应该这样理解：

- “给我看最近 10 条刚进库的文献” -> `library recent --count`
- “先预览再合并这两篇” -> `item merge`
- “先找重复，再批量合并” -> `library duplicates` / `duplicates-merge`

更完整的 agent 路由，见：

- [Skills 总览](/skills/overview)
- [路由策略](/skills/routing)
- [CLI 总览](/cli/overview)
