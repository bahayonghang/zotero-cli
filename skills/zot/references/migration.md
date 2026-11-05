# 从参考实现迁移

遇到旧命令名时按这里映射。映射里没有的命令不要编造，也不要补一套新的 flat alias。

## 从 ref\zotero-cli 迁移过来时怎么理解

- `search` -> `library search`
- `get` -> `item get`
- `open` -> `item open`
- `open --url` -> `item open --url`
- `annotations` -> `item annotation list` 或 `item pdf --annotations`
- `notes` -> `item note list`
- `notes search` -> `item note search`
- `collections` -> `collection list`
- `collection <id>` -> `collection items <id>`
- `add doi` / `add url` -> `item add-doi` / `item add-url`
- `tags` -> `library tags`
- `stats` -> `library stats`
- `recent [n]` -> `library recent --count <n>`
- `merge` -> `item merge`；如果是先找重复再合并，走 `library duplicates` / `duplicates-merge`

不要迁回去的旧心智：

- 不补 flat top-level alias
- 不补 `--api-base`
- 不补 compact JSON 默认输出
- 不把 connector 风格 `search` / `fetch` 重新做成另一套主命令

## 从 ref\zotagent 迁移过来时怎么理解

先记住两点：

- 当前 `zot` 没有照搬 `zotagent` 的 flat command 面
- 当前 `sync` 只做 preprint publication status，同名但不是附件索引器

已有覆盖或可替代的部分：

- DOI / URL / 文件导入：`item add-doi` / `item add-url` / `item add-file` / `item create`
- 库级语义检索：`library semantic-index` / `library semantic-search`
- 单篇 PDF 提取：`item pdf` / `item fulltext` / `item outline`
- citation key 入口：先 `library citekey`，再转到 `item get` / `item cite`
- 撤稿覆盖：`item scite retractions` / `item scite report` / `item scite search`（zotagent 通常没内置，这是 zot 多出来的能力，不是迁移）
- 回收站枚举与版本增量：`item deleted` / `item versions --since`（替代 zotagent 风格的轮询脚本）

当前没补齐，不能假装存在：

- `s2`
- `add --s2-paper-id`
- `search-in`
- `metadata`
- `read`
- `expand`
- zotagent 风格 `status`
- zotagent 风格 `sync` 全量附件索引
- 按 `title` / `author` / `year` / `publication` 一次性手工建条目

遇到这些请求时怎么处理：

- `search-in` / `expand`：明确说当前没有等价命令，只能先 `item fulltext` / `item pdf` 拉文本，再做 agent 侧二次定位
- `metadata`：标题用 `library search --title`，期刊用 `--journal`（`publicationTitle` 或 `journalAbbreviation`），主题词用 `--topic`（标题、摘要或标签，不是语义，也不是 PDF 正文）。桌面读做不到的条件组合要说明，不要丢掉过滤条件
- `status`：用 `doctor` + `library semantic-status` 组合回答，不要把 `sync update-status` 说成索引状态
- `s2` / `--s2-paper-id`：直接说明当前未实现，不要伪造替代命令
- zotagent `sync`：说明当前只能用 `library semantic-index --fulltext` 或 `workspace index` 做部分替代，而且范围主要是 metadata + PDF
