# 从 `ref/zotagent` 迁移

这页回答三件事：

1. `ref/zotagent` 现在实现了什么
2. 当前 Rust `zot` 已经覆盖到哪一步
3. 还没补齐的能力准备怎么补

## 先看结论

| 类型 | 结论 |
| --- | --- |
| 已覆盖 | DOI / URL / 文件导入、citation key 入口、单篇 PDF 提取、库级 semantic index/search |
| 部分替代 | 本地附件索引、全文检索、索引状态查看 |
| 明确未实现 | `s2`、`add --s2-paper-id`、`search-in`、`metadata`、`read`、`expand`、zotagent 风格 `status`、zotagent 风格 `sync`、basic-field 一次性建条目 |
| 当前仓库额外能力 | notes、tags、collections、saved searches、feeds、annotation 创建、workspace、Scite、duplicates merge、profile config、publication-status sync |

## `zotagent` 实现了什么

按 `ref/zotagent/README.md` 和 `ref/zotagent/src/cli.ts`，它当前主打四类能力：

1. `add`
   - 支持 DOI 导入
   - 支持按基础字段直接建条目
   - 支持 `s2` 搜索后按 `paperId` 导入
2. `sync` / `status`
   - 扫描 Zotero 附件
   - 提取并索引 PDF / EPUB / HTML / TXT
   - 查看本地索引状态和路径
3. `search`
   - 默认做 FTS5 关键词全文检索
   - `--semantic` 做向量检索
   - `search-in` 做单篇内检索
   - `metadata` 做 bibliography 字段检索
4. `read`
   - `read` 返回块级片段
   - `fulltext` 返回清洗后的全文
   - `expand` 返回命中块上下文
   - 都支持 `itemKey` / `citationKey`

## 当前 `zot` 对照

| `zotagent` 能力 | 当前 `zot` | 状态 | 说明 |
| --- | --- | --- | --- |
| DOI / URL 导入 | `item add-doi` / `item add-url` | 已覆盖 | 还支持 `--attach-mode` |
| 文件导入 | `item add-file` / `item create --pdf` | 已覆盖 | 还能带 `--doi` 补 metadata |
| citation key 入口 | `library citekey` | 已覆盖 | 先解析 citekey，再转别的 `item` 流程 |
| 单篇 PDF 全文 / outline / 批注读取 | `item pdf` / `item fulltext` / `item outline` | 已覆盖 | 当前主要针对 PDF attachment |
| 库级向量检索 | `library semantic-index` / `library semantic-search` | 已覆盖 | 支持 `bm25` / `semantic` / `hybrid` |
| 手工工作区索引与问答 | `workspace index` / `workspace query` | 已覆盖 | 这是 `zotagent` 没有的工作面 |
| basic-field 一次性建条目 | 无 | 未覆盖 | 现在只能 DOI / URL / PDF / 文件导入后再 update |
| `s2` 搜索 | 无 | 未覆盖 | 当前没有 Semantic Scholar 搜索命令面 |
| `add --s2-paper-id` | 无 | 未覆盖 | 当前不能按 `paperId` 直接导入 |
| Zotero 附件全量索引 `sync` | 无等价命令 | 部分替代 | `library semantic-index --fulltext` 只覆盖 metadata + PDF |
| 索引状态 `status` | `doctor` + `library semantic-status` | 部分替代 | 不是一个等价命令，也不含 zotagent 那套路径 / 错误统计 |
| 默认全文关键词检索 | 无等价命令 | 部分替代 | `library semantic-search --mode bm25` 依赖先建 semantic index，且不是 zotagent 的 FTS5 语法 |
| 单篇内检索 `search-in` | 无 | 未覆盖 | 当前只能先拉全文再做 agent 侧二次定位 |
| bibliography 字段检索 `metadata` | 无 | 未覆盖 | `library search` 有 filter，但不是 field-scoped metadata search |
| 块级读取 `read` | 无 | 未覆盖 | 当前没有 block index 和块窗口读取 |
| 命中扩展 `expand` | 无 | 未覆盖 | 当前没有 block-based context expansion |
| `citationKey` 直接驱动 `fulltext` / `expand` | 无 | 未覆盖 | 现在要先 `library citekey` 再转 item key |
| 多附件合并成一个逻辑文档 | 无 | 未覆盖 | 当前读面基本围绕首个 PDF attachment |

## 最大的语义差异

### 1. `sync` 同名但不是一回事

- `zotagent sync`：附件提取 + 索引
- `zot sync update-status`：查 preprint 是否已正式发表

这块不能靠别名糊过去。当前仓库里 `sync` 已经有稳定语义，不该把附件索引重新塞进去。

### 2. 现有检索底座是 `RagIndex`，不是 `zotagent` 的 FTS5 文档面

当前仓库已经有：

- `library semantic-index --fulltext`
- `library semantic-search --mode bm25|semantic|hybrid`
- `workspace index` / `workspace query`

但这条链路主要服务：

- metadata chunk
- PDF chunk
- RAG / semantic retrieval

它还不是：

- 单文档块级读写面
- `search-in`
- `expand`
- bibliography metadata field 检索
- PDF / EPUB / HTML / TXT 的统一附件抽取面

### 3. Rust 版已经比 `zotagent` 多了不少 Zotero 原生工作流

当前仓库额外有：

- notes / tags / collections 的完整读写
- saved search
- feeds
- annotation 创建
- duplicates / merge
- workspace
- Scite
- profile config
- publication-status sync

所以补齐 `zotagent` 缺口时，重点不是“照抄命令名”，而是把缺失能力接进现有 `library` / `item` / `workspace` 结构里。

## 补全 plan

### 阶段 1：先补索引底座

目标：

- 把现在偏 PDF + RAG 的索引底座，扩成稳定的附件全文底座

主要工作：

- 在 `zot-local` 抽象统一 extractor，纳入 PDF / EPUB / HTML / TXT
- 给索引层补持久化 manifest / extraction status / error catalog
- 让一个条目的多附件能组成统一逻辑文档
- 新增只读状态接口，至少能回答：索引路径、附件数、错误数、最近更新时间

约束：

- 不改现有 `sync update-status` 语义
- 不把持久索引写进临时目录

验证：

- fixture 覆盖 PDF / EPUB / HTML / TXT 四类附件
- 重跑索引时能跳过未变更文件
- 索引状态能稳定输出

### 阶段 2：再补读面和检索面

目标：

- 给 agent 一个诚实的全文读取 / 命中扩展 / 单篇检索入口

主要工作：

- 增加单篇内检索命令，等价覆盖 `search-in`
- 增加块级读取命令，等价覆盖 `read`
- 增加命中扩展命令，等价覆盖 `expand`
- 增加 field-scoped metadata search
- 给文本读取命令补 `citationKey` 选择器，避免先手动二跳

命名原则：

- 保持 `library` / `item` 分层
- 不恢复 flat top-level alias

验证：

- 命中块、上下文半径、citation key 路由都要有集成测试
- 多附件条目要验证块序号稳定

### 阶段 3：最后补导入和 Semantic Scholar 缺口

目标：

- 补齐 `zotagent add` 里当前最缺的两块

主要工作：

- 增加 Semantic Scholar 搜索命令
- 增加按 `paperId` 导入
- 增加 basic-field 一次性建条目入口

约束：

- 继续只通过 Zotero Web API 写库
- 保持现有 JSON envelope

验证：

- DOI / paperId / manual metadata 三条导入主线都要有回归
- 没有写凭据时要停在清晰报错

### 阶段 4：收尾 skill、docs、回归

目标：

- 让 agent 和文档只承诺真实能力

主要工作：

- 更新 `skills/zot-skills/SKILL.md`
- 增加 ref\zotagent 迁移 prompt / eval
- 同步中英文 docs

验证：

- 文档站点构建通过
- skill 回归样例覆盖 `search-in` / `status` / `s2` 三类缺口

## 现阶段给 agent 的执行边界

在这些能力落地前，正确做法是：

- 要 `search-in` / `expand`：先说当前没原生命令，再退到 `item fulltext` / `item pdf`
- 要 `status`：用 `doctor` + `library semantic-status`
- 要 `s2` / `paperId` 导入：直接说明当前没实现
- 要 zotagent 式 `sync`：明确当前只有 `library semantic-index --fulltext` 或 `workspace index` 的部分替代

关键点只有一个：不要把计划当已实现能力写进 skill 和 docs。
