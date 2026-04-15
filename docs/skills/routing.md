# 路由策略

按用户意图选择命令族：

| 用户想做什么 | 首选命令 | 说明 |
| --- | --- | --- |
| 找论文、看条目、查重复、看最近新增 | `library ...` | 本地只读，默认起点 |
| 读某篇的元数据、附件、PDF、引用 | `item ...` | 适合单条目精读 |
| 改标题、打标签、写笔记、上传附件 | `item ...` | 需要写权限 |
| 看 collection、整理 collection | `collection ...` | 读写混合 |
| 按主题组织一批论文并持续检索 | `workspace ...` | 本地工作区，不直接改 Zotero collection |
| 对 workspace 做 RAG / 语义检索 | `workspace index/query` | embedding 不可用时自动退化 |
| 检查 preprint 是否已发表 | `sync update-status` | `--apply` 前要确认 |

## 一句话判断

- 单篇或少量条目：优先 `library` / `item`
- 一组主题文献：优先 `workspace`
- 会改库：优先确认写权限，再走 `item` / `collection` / `sync`

## 启动顺序

1. 先决定调用路径：`zot ...` 或 `cargo run -q -p zot-cli -- ...`
2. 新环境、写操作、PDF、workspace、异常场景先跑 `doctor`
3. 整轮任务保持同一种调用路径
