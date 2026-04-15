---
name: zot-skills
description: 当用户想在本机已有的 Zotero 文献库或 reading workspace 上完成实际操作时，必须使用这个 skill，并把 Rust `zot` CLI 作为默认执行面，即使用户没有显式提到 `zot`。适用于库内搜索/浏览条目、按 citation key/tag/creator/year 查找、读取 metadata/fulltext/PDF/outline/children/annotations、管理 tags/notes/collections、查看 libraries/feeds、建立或查询 workspace、做 semantic index/search、运行 `doctor`、做 Scite 检查、或执行明确授权的 Zotero Web API 写操作。不要把它用于泛化找论文、论文总结、普通 bibliography 指导、非 Zotero 的 PDF 阅读/整理，除非用户明确要把结果落到 `zot` / Zotero 库 / workspace 中。
---

# zot-skills

这个 skill 的工作不是背命令，而是把用户的 Zotero 任务稳定地路由到本仓库的 Rust `zot` CLI。

优先级规则：

- 只要任务的真实目标是操作**已有的本地 Zotero 库**或**已有/将创建的 reading workspace**，就用本 skill，即使用户没说 `zot`。
- 如果用户只是想“找论文”“总结论文”“讲讲引用格式”“读一个不在 Zotero 里的 PDF”，默认不要用本 skill。
- 把 `zot` 视为唯一执行面。`ref/` 里的旧参考实现和 `zot mcp serve` 都不是当前可依赖入口。

## 先判断什么

- 这是只读任务，还是会改 Zotero 库。
- 这是单条/少量条目操作，还是要建一个持续使用的 workspace。
- 这是字段搜索，还是 citation key / semantic / annotation / Scite 这类专门工作流。
- 是否需要先跑 `doctor`。

## 路由矩阵

| 任务 | 首选命令面 | 说明 |
| --- | --- | --- |
| 关键词、tag、creator、year、collection 搜索 | `library search` | 默认的库内只读入口 |
| 按 citation key 查条目 | `library citekey` | 先走 Extra fallback；Better BibTeX 可用时自动补强 |
| 看 tags / libraries / feeds | `library tags` / `library libraries` / `library feeds` | feeds 不走 `--library` scope 切换 |
| 看某个 feed 的条目 | `library feed-items <library_id>` | feed library id 来自 `library libraries` 或 `library feeds` |
| 看元数据、children、fulltext、PDF、outline、引用 | `item ...` | 单篇条目主入口 |
| 新增条目（DOI / URL） | `item add-doi` / `item add-url` | 支持 `--attach-mode` |
| 上传本地文件建条目 | `item add-file` | 可选 `--doi` 辅助建条目；**不支持** `--attach-mode` |
| 管 notes / tags / annotations | `item note ...` / `item tag ...` / `item annotation ...` | 写操作要过安全门 |
| 看 collection / 调整 collection 成员 | `collection ...` | `collection search` 常用于先定位 |
| 建主题工作区并长期维护 | `workspace new` / `add` / `import` / `index` / `query` | workspace 不等于 Zotero collection |
| 已有 workspace 的关键词检索 | `workspace search` | 文本/关键词检索，不是问答 |
| 已有 workspace 的问答式检索 | `workspace query` | 依赖索引；优先用于问答 |
| 库级 semantic 状态 / 建索引 / 检索 | `library semantic-status` / `semantic-index` / `semantic-search` | 与 workspace 索引共享底层索引能力 |
| 查重复 / 合并重复 | `library duplicates` / `duplicates-merge` | merge 默认先 dry-run，`--confirm` 才执行 |
| Scite tally / retractions | `item scite ...` | 基于库内条目，不改写成网页搜索 |
| 检查 preprint 是否已正式发表 | `sync update-status` | `--apply` 有副作用 |

## 调用顺序

1. 如果系统已安装 `zot`，优先用 `zot --json ...`。
2. 只有在开发仓库环境且 `zot` 不在 `PATH` 时，才退回：

```bash
cargo run -q -p zot-cli -- ...
```

3. 同一轮任务保持同一种调用方式，不要来回切换。

## 诊断门

以下场景默认先跑 `doctor`：

- 第一次接触这个环境
- 任何写操作
- PDF / outline / annotation 相关任务
- semantic index / semantic search / workspace query
- citation key 查询
- 用户说“为什么不工作”

首选：

```bash
zot --json doctor
```

开发环境 fallback：

```bash
cargo run -q -p zot-cli -- --json doctor
```

重点看这些字段：

- `db_exists`: 本地 Zotero 数据是否可读
- `write_credentials.configured`: 是否允许 Web API 写操作
- `pdf_backend.available`: 是否支持 PDF 文本和 outline 相关能力
- `better_bibtex.available`: citation key 直查是否有 BBT 支持
- `libraries.feeds_available`: feeds 是否可读
- `semantic_index`: 当前库级 semantic index 是否已存在
- `annotation_support.pdf_outline` / `annotation_support.annotation_creation`: outline 与 PDF annotation 是否可用
- `embedding.configured`: semantic search 和 workspace hybrid query 是否具备 embedding

## 硬约束与安全门

真实约束：

- `--library` 只接受 `user` 或 `group:<id>`。
- workspace 名必须是 kebab-case，例如 `llm-safety`。
- `zot mcp serve` 目前不可用，不要把它纳入备选路径。
- `item add-file` 不支持 `--attach-mode`。
- `item annotation create` / `create-area` 只适用于 PDF attachment。

默认视为有副作用的动作：

- `item create`
- `item add-doi`
- `item add-url`
- `item add-file`
- `item update`
- `item trash`
- `item restore`
- `item attach`
- `item note add`
- `item note update`
- `item note delete`
- `item tag add`
- `item tag remove`
- `item tag batch`
- `item annotation create`
- `item annotation create-area`
- `collection create`
- `collection rename`
- `collection delete`
- `collection add-item`
- `collection remove-item`
- `library duplicates-merge --confirm`
- `sync update-status --apply`

执行规则：

1. 用户只是“看看”“分析”“评估”时，不要偷偷写库。
2. 普通、单项、可逆写操作，在用户明确要求后可以执行。
3. 对这些高风险动作，先总结即将发生的变化，再执行：
   - `item trash`
   - `item note delete`
   - `collection delete`
   - `library duplicates-merge --confirm`
   - `sync update-status --apply`
4. 永远不要直接修改 `zotero.sqlite`。Rust 版写路径只走 Web API。

## 常见语义差异

- `workspace search`: 在已存在 workspace 内做文本/关键词检索。
- `workspace query`: 对已索引 workspace 做问答式检索。
- `library semantic-search`: 直接针对库级 semantic index，不等价于 workspace query。
- `item add-doi` / `item add-url` / `item create --doi|--url|--pdf`: 支持 `--attach-mode auto|linked-url|none`。
- `item add-file`: 上传本地文件，可能结合 `--doi` 建出更好元数据，但不接受 `--attach-mode`。
- feeds 不通过 `--library group:<id>` 访问，使用 `library feeds` / `library feed-items <library_id>`。

## 高价值模板命令

```bash
zot --json library search "reward hacking" --limit 10
zot --json library citekey Smith2024
zot --json item get ATTN001
zot --json item note add ATTN001 --content "Key finding: ..."
zot --json item tag add ATTN001 --tag priority
zot --json item add-doi 10.1038/s41586-023-06139-9 --tag reading --attach-mode auto
zot --json item add-file paper.pdf --doi 10.1038/nature12373 --tag imported
zot --json workspace new mechinterp
zot --json workspace import mechinterp --search "mechanistic interpretability"
zot --json workspace index mechinterp
zot --json workspace query mechinterp "What methods are used to identify circuits?" --mode hybrid --limit 5
zot --json library semantic-status
zot --json library semantic-search "mechanistic interpretability methods" --mode hybrid --limit 10
zot --json library feeds
zot --json library feed-items 3 --limit 20
```

## 失败时的 fallback

- 没有 `zot`：在开发仓库里退回 `cargo run -q -p zot-cli -- ...`；普通环境直接说明 CLI 不在 `PATH`
- 没有写权限：停在只读分析，并告诉用户缺 `ZOT_API_KEY` / `ZOT_LIBRARY_ID`
- 没有 Better BibTeX：`library citekey` 仅做 Extra fallback；未命中时要明确说明，不要假装查过 BBT
- 没有 Pdfium：不要承诺 fulltext / outline / text-position / annotation
- 没有 embedding：semantic 检索要说明会降级；workspace 问答改用 `--mode bm25`
- `attach-mode auto` 没找到 OA PDF：条目仍可能创建成功，这不是硬错误

## 输出契约

最终回答应该：

- 先回答用户真正的问题，而不是先贴命令
- 再给关键证据、已执行动作或失败原因
- 如果失败，明确告诉用户缺什么、下一步是什么
- 默认不要倾倒 raw JSON；先读 envelope，再转述有效信息

这个 skill 的目标不是展示命令面有多大，而是把 Zotero / workspace 任务路由到**最稳、最短、最少副作用**的 `zot` 工作流。
