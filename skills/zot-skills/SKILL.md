---
name: zot-skills
description: Use this skill whenever the user mentions Zotero, 文献库, papers, references, citations, bibliography, PDF attachments, collections, tags, notes, literature organization, reading workspaces, paper RAG, or wants to search/read/export/update an academic library. Also trigger when the user asks to “找论文”, “导出引用”, “整理 Zotero”, “做阅读工作区”, “查 PDF 内容”, or “同步 preprint 状态”, even if they never explicitly say “Zotero CLI”.
---

# zot-skills

用本仓库里的 Rust `zot` CLI 作为 Zotero 相关任务的默认执行面。这个 skill 的重点不是背命令，而是帮模型快速判断：

- 这是一次性查文献，还是要搭一个持续使用的 workspace
- 这是本地只读任务，还是需要走 Zotero Web API 的写操作
- 什么时候该先 `doctor`，什么时候可以直接执行
- 什么时候必须停下来做安全确认

## 快速路由

按用户意图选命令族：

| 用户想做什么 | 首选命令 | 备注 |
|---|---|---|
| 找论文、看条目、查重复、看最近新增 | `library ...` | 本地只读，默认先用这个 |
| 读某篇的元数据、附件、PDF、引用 | `item ...` | `get / pdf / export / cite / open` |
| 改标题、打标签、写笔记、上传附件 | `item ...` | 需要写权限 |
| 看 collection、整理 collection | `collection ...` | 读写混合，删除前要确认 |
| 按主题组织一批论文并做后续检索 | `workspace ...` | 本地 workspace，不改 Zotero collection |
| 对 workspace 做 RAG / 语义检索 | `workspace index/query` | 未配置 embedding 时自动退化为 BM25 |
| 检查 preprint 是否已正式发表 | `sync update-status` | `--apply` 前要确认 |

一句话判断：

- 单篇或少量条目：优先 `library` / `item`
- 一组主题文献：优先 `workspace`
- 任何会改库的动作：先看写权限，再走 `item` / `collection`

## 启动顺序

1. 先决定用哪个启动方式：
   - 如果系统里已有 `zot`，优先直接用它
   - 如果没有，就用 `cargo run -q -p zot-cli -- ...`
2. 只要涉及下面任一情况，先跑 `doctor`：
   - 第一次接触这个环境
   - 要写 Zotero
   - 要读 PDF
   - 要做 workspace query / embedding
   - 用户说“为什么不工作”
3. 选定一种调用方式后，整轮任务保持一致，不要一会儿 `zot` 一会儿 `cargo run`

## 诊断门

优先执行：

```bash
zot --json doctor
```

如果 `zot` 不在 `PATH`：

```bash
cargo run -q -p zot-cli -- --json doctor
```

重点看：

- `data_dir` 和 `db_exists`：本地 Zotero 数据是否可读
- `write_credentials.configured`：能不能做写操作
- `pdf_backend.available`：PDF 文本/批注能不能提取
- `embedding.configured`：workspace query 是否能用语义检索
- `semantic_scholar.configured`：preprint 状态检查是否有更高限额

## 安全门槛

下面这些动作默认视为有副作用：

- `item create`
- `item update`
- `item trash`
- `item restore`
- `item attach`
- `item note add/update`
- `item tag add/remove`
- `collection create/rename/delete/add-item/remove-item`
- `sync update-status --apply`

执行规则：

1. 如果用户已经明确要求执行这些动作，直接做。
2. 如果任务只是“分析”“看看”“评估一下”，不要偷偷写库。
3. 对明显破坏性的动作，先确认用户意图明确：
   - `item trash`
   - `collection delete`
   - `sync update-status --apply`
4. 永远不要尝试直接改 `zotero.sqlite`。Rust 版 `zot` 的写路径只走 Web API。

## 读写边界

- 读操作来自本地 Zotero 数据目录：
  - `zotero.sqlite`
  - 附件 storage 目录
- 写操作来自 Zotero Web API：
  - 需要 `ZOT_API_KEY`
  - 需要 `ZOT_LIBRARY_ID`
- group library 用：
  - `--library group:<id>`

如果用户要写，但 `doctor` 显示 credentials 缺失，就停在“解释缺什么 + 给出下一步”，不要编造成功结果。

## 响应约定

1. 默认优先 `--json`，尤其是搜索、筛选、workspace 查询、脚本化任务。
2. 只有当用户明确要“直接给我引用”或“直接打印出来”时，才用非 JSON 输出。
3. 不要把一大段原始 JSON 直接甩给用户；先读结果，再回答用户真正的问题。
4. 如果命令失败，优先转述：
   - 错误 code
   - message
   - hint

## 常用命令模板

### 1. 查找与浏览

```bash
zot --json library search "transformer attention" --limit 10
zot --json library list --limit 20
zot --json library recent <YYYY-MM-DD> --limit 20
zot --json library stats
zot --json library duplicates --limit 20
```

### 2. 单条目读取

```bash
zot --json item get ATTN001
zot --json item related ATTN001 --limit 10
zot item open ATTN001
zot item open ATTN001 --url
zot --json item pdf ATTN001
zot --json item pdf ATTN001 --annotations
zot item export ATTN001 --format bibtex
zot item export ATTN001 --format ris
zot item export ATTN001 --format csl-json
zot item cite ATTN001 --style apa
zot item cite ATTN001 --style nature
zot item cite ATTN001 --style vancouver
```

### 3. 写操作

```bash
zot --json item create --doi 10.1038/s41586-023-06139-9
zot --json item create --url https://arxiv.org/abs/2301.00001
zot --json item create --pdf paper.pdf
zot --json item update ATTN001 --title "New Title" --field publicationTitle=Nature
zot --json item trash ATTN001
zot --json item restore ATTN001
zot --json item attach ATTN001 --file supplement.pdf
zot --json item note list ATTN001
zot --json item note add ATTN001 --content "Key finding: ..."
zot --json item note update NOTE001 --content "Revised note"
zot --json item tag list ATTN001
zot --json item tag add ATTN001 --tag important --tag reading-list
zot --json item tag remove ATTN001 --tag obsolete
```

### 4. Collections

```bash
zot --json collection list
zot --json collection items COLL001
zot --json collection create "New Project"
zot --json collection rename COLL001 "Renamed Project"
zot --json collection delete COLL001
zot --json collection add-item COLL001 ATTN001
zot --json collection remove-item COLL001 ATTN001
```

### 5. Workspaces 与 RAG

```bash
zot --json workspace new llm-safety --description "LLM safety papers"
zot --json workspace list
zot --json workspace show llm-safety
zot --json workspace add llm-safety ATTN001 ATTN002
zot --json workspace remove llm-safety ATTN001
zot --json workspace import llm-safety --collection COLL001
zot --json workspace import llm-safety --tag safety
zot --json workspace import llm-safety --search "reward hacking"
zot --json workspace search llm-safety "alignment"
zot workspace export llm-safety --format markdown
zot workspace export llm-safety --format json
zot workspace export llm-safety --format bibtex
zot --json workspace index llm-safety
zot --json workspace query llm-safety "What are the main causes of reward hacking?" --mode hybrid --limit 5
```

### 6. Preprint 状态同步

```bash
zot --json sync update-status ATTN001
zot --json sync update-status --collection COLL001 --limit 20
zot --json sync update-status --apply --limit 20
```

## 典型工作流

### 工作流 A：用户只是想“帮我找论文并给个引用”

按这个顺序：

1. `library search`
2. `item get`
3. `item cite` 或 `item export`

最小示例：

```bash
zot --json library search "reward hacking" --limit 5
zot --json item get ATTN001
zot item cite ATTN001 --style apa
```

### 工作流 B：用户想“做一个主题工作区，后面持续问答”

按这个顺序：

1. `workspace new`
2. `workspace import` 或 `workspace add`
3. `workspace index`
4. `workspace query`

最小示例：

```bash
zot --json workspace new mechinterp --description "Mechanistic interpretability papers"
zot --json workspace import mechinterp --search "mechanistic interpretability"
zot --json workspace index mechinterp
zot --json workspace query mechinterp "What methods are used to identify circuits?" --limit 5
```

### 工作流 C：用户想“直接改 Zotero”

按这个顺序：

1. `doctor`
2. 确认有写权限
3. 执行写命令
4. 明确告诉用户改了什么 key / collection

最小示例：

```bash
zot --json doctor
zot --json item tag add ATTN001 --tag priority
zot --json collection add-item COLL001 ATTN001
```

## 失败时的 fallback

- 没有 `zot`：退回 `cargo run -q -p zot-cli -- ...`
- 没有写权限：停在只读分析，并告诉用户缺 `ZOT_API_KEY` / `ZOT_LIBRARY_ID`
- 没有 Pdfium：不要承诺能提 PDF 文本或批注
- 没有 embedding：继续用 `workspace query --mode bm25` 或让 `hybrid` 自然退化
- `zot mcp serve`：当前 Rust 版未实现，不要把它当可用能力
- `item create --pdf` 抽不出 DOI：要求显式传 `--doi`，不要猜元数据

## 不要这样做

- 不要直接修改本地 SQLite
- 不要在“只是看看”的任务里偷偷执行写操作
- 不要为了显得完整而把所有命令都跑一遍
- 不要把 raw JSON 原样倾倒给用户
- 不要假设 MCP server 已经可用

## 输出目标

最终给用户的回答应该是：

- 先回答问题本身
- 再补充关键证据或已执行动作
- 如果失败，明确说失败原因和下一步

skill 的目标不是展示你记住了多少命令，而是让 Zotero 任务更稳、更快、更少走弯路。
