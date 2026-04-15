# 路由策略

按用户意图选择命令族：

| 用户想做什么 | 首选命令 | 说明 |
| --- | --- | --- |
| 按 query / tag / creator / year / collection 找条目 | `library search` | 默认只读入口 |
| 按 citation key 定位文献 | `library citekey` | Better BibTeX 可用时会补强 |
| 看 tags / libraries / feeds / feed items | `library tags` / `libraries` / `feeds` / `feed-items` | feeds 不走 `--library` |
| 做 semantic index / semantic search | `library semantic-*` | 先看 doctor 和 embedding |
| 读单条目的 metadata、PDF、children、outline | `item ...` | 单条目精读入口 |
| 新增 DOI / URL / 文件条目 | `item add-doi` / `add-url` / `add-file` | `item create` 仍兼容旧用法 |
| 管 notes / tags / annotations / Scite | `item note ...` / `item tag ...` / `item annotation ...` / `item scite ...` | annotation 创建有前置条件 |
| 搜 collection 或维护 collection | `collection ...` | 真实 Zotero collection 读写 |
| 围绕主题建立长期 paper set | `workspace ...` | 本地工作区，不直接改 Zotero collection |
| 检查 preprint 是否已发表 | `sync update-status` | `--apply` 前要确认 |

## 一句话判断

- 单篇或少量条目：优先 `library` / `item`
- 需要 citation key / feeds / semantic / annotation / Scite：直接进对应高级子命令
- 一组主题文献：优先 `workspace`
- 会改库：先确认写权限，再走 `item` / `collection` / `sync`

## 启动顺序

1. 决定调用路径：`zot ...` 或 `cargo run -q -p zot-cli -- ...`
2. 新环境、写操作、PDF、semantic、BBT、异常场景先跑 `doctor`
3. 整轮任务保持同一种调用路径
