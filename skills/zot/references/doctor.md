# 诊断、约束与失败处理

入口决定何时运行 doctor，以及哪四项能力改变路由。这里是字段含义、硬约束、容易混淆的命令和失败后的停止方式。

### 8. 配置排障

用户通常会说：

- “为什么这个环境不能写 Zotero”
- “先帮我看看配置是不是对的”
- “把当前 profile 切到 work”
- “初始化一个新的 config profile”

优先路由：

- 诊断：`doctor`
- 配置：`config show` / `init` / `set` / `profiles list` / `profiles use`

connector 不需要 API key 或插件配置。`doctor` 显示 `connector_write` 不可用时，先检查 Zotero 是否正在运行以及本机 connector 是否可达；不要通过修改 profile 把 connector 扩成通用写后端。

## 诊断门

以下场景默认先跑 `doctor`：

- 第一次接触这个环境
- 任何写操作
- PDF / outline / annotation / attachment 相关任务
- semantic index / semantic search / workspace query
- citation key 查询
- saved search / 配置排障 / profile 切换
- 用户说“为什么不工作”

`item get`、`item children`、`item pdf`、`library search`、`library citekey`，以及 `collection list` / `get` / `subcollections` / `search` / `items` / `item-count` / `tags` 和 `library list` / `stats` / `tags` 不必先跑完整 doctor 就能读。doctor 仍用于第一次接触环境、写操作、PDF 排障和“为什么不工作”。SQLite snapshot 单独失败（库被 Zotero 锁住）不要中止这些读取，也不要把「先关闭 Zotero」说成这十条命令的下一步：看 `capabilities.sqlite_snapshot` 和 `capabilities.desktop_read` 的 `next`，再执行读命令。非空通用 `library search` 仍先读快照。探针可达不是条目或 PDF 已经读成功。`desktop_read.item_read_verified` 和 `pdf_read_verified` 恒为 false。

首选：

```bash
zot --json doctor
```

开发环境 fallback：

```bash
cargo run -q -p zot-cli -- --json doctor
```

重点看这些字段：

- `db_exists`
- `capabilities.local_sqlite_read`
- `capabilities.sqlite_snapshot`（快照能不能打开，以及这条 CLI 下一步能做什么）
- `capabilities.desktop_read`（运行中的桌面读能不能走；探针成功不是条目或 PDF 已读）
- `capabilities.local_sqlite_read.fulltext.legacy_tables`
- `capabilities.local_sqlite_read.fulltext.sidecar_present`
- `capabilities.local_api_write`（版本/可达/凭据存在分别报告，`verified: false` 不表示已验证可写）
- `capabilities.local_http_read`
- `capabilities.connector_write`（仅表示本机 BibTeX/RIS import 能力）
- `capabilities.web_write.configured`（只表示凭据存在）
- `capabilities.web_write.verified`（当前为 `false`，doctor 不联网验证 key/scope/permission）
- `write_credentials.configured`（只表示 Web API credential，不是 connector 导入能力）
- `pdf_backend.available`
- `better_bibtex.available`
- `libraries.feeds_available`
- `semantic_index`
- `annotation_support`
- `embedding.configured`
- `config_file`

## 硬约束

- `--library` 只接受 `user` 或 `group:<id>`
- `--json` 是 global flag，必须放在子命令前（例 `zot --json item get K`），不能写成 `zot item --json get K`
- workspace 名必须是 kebab-case
- workspace 文件由 `AppConfig::state_dir().join("workspaces")` 解析，不要把 `~/.config/zot/workspaces` 写成所有平台的路径。Linux/XDG 示例：`~/.config/zot/workspaces/<name>.toml`；macOS：`~/Library/Application Support/zot/workspaces/<name>.toml`；Windows：`%AppData%\zot\workspaces\<name>.toml`。运行时以 `doctor` / `config show`（`doctor.data.config_file`）为准。索引副文件 `<name>.idx.sqlite`，PDF cache 副文件 `.md_cache.sqlite`
- `zot mcp serve` 当前不可用
- `item add-file` 不支持 `--attach-mode`
- `item annotation create` / `create-area` 只适用于 PDF attachment，且 attachment 的 `content_type` 必须是 `application/pdf`，否则报 `attachment-not-pdf`
  - `item annotation create` 支持 `--occurrence N`（0.5.0 起，默认 1），用于在同一页出现多次的同一文本中选中第 N 个。返回 JSON 含 `occurrence` / `total_matches` / `more_occurrences`，可用于连锁调用。
- `library saved-search` 处理的是保存查询的条件，不是结果项
- `library saved-search create --conditions` 必须是 JSON 数组，每项形如 `{"condition": "...", "operator": "...", "value": "..."}`，至少一条；空数组直接报 `saved-search-conditions`
- Pdfium 路径覆盖：`ZOT_PDFIUM_LIB_PATH` / `PDFIUM_LIB_PATH` 指向 lib，`ZOT_PDFIUM_CACHE_DIR` 覆盖自动下载缓存目录
- 永远不要直接修改 `zotero.sqlite`
- Zotero 10+ 的本地内容操作走授权 Local API；旧版 Local HTTP 只读。Connector 仍仅支持 item import。
- connector 无鉴权但仅监听 loopback，只支持向 Zotero UI 当前选中目标新增 BibTeX/RIS 条目；不能用于 merge/dedupe 或其他 mutation
- API key 和 raw merge plan token 不得进入输出、日志、fixture、文档示例或 eval

## 常见语义差异

- `workspace search` 是关键词检索，`workspace query` 是问答检索
- `library recent --count 10` 是最近 N 条，`library recent 2026-04-01 --limit 20` 是按时间边界筛
- `library semantic-search` 是库级语义检索，不等价于 workspace query
- `item add-doi` / `item add-url` / `item create --doi|--url|--pdf` 支持 `--attach-mode`
- `item add-file` 可以带 `--doi` 补元数据，但不接受 `--attach-mode`
- feeds 不通过 `--library group:<id>` 访问，而是用 `library feeds` / `feed-items`
- `item download` 下载本地附件文件，默认不覆盖已有目标；只有复核现有文件后才加
  `--force`。`item attach` 上传新附件
- `item merge` 是手工选 keeper/source 的通用合并，`library duplicates-merge` 是先找重复、再按 keeper 合并
- `library dedupe` 是整库/整 collection 自动选 keeper 的批量清理，`library duplicates-merge` 是单组手工指定 keeper 的合并
- `config show` 是看有效配置，`config profiles use` 是切换默认 profile

## 失败时的 fallback

- 没有 `zot`：开发仓库里退回 `cargo run -q -p zot-cli -- ...`
- Zotero 未运行或 connector 不可达：停下并请用户启动 Zotero、确认本机 connector 可达后重试原 import；不改走 Web
- connector 目标只读：点名当前目标不可写，请用户在 Zotero UI 选择可写 collection/library 后重新 dry-run；不改走 Web
- Web 写不可用：明确告诉用户缺 `ZOT_API_KEY` / `ZOT_LIBRARY_ID`；connector 不能替代其他 mutation
- 目标路径不支持该 mutation：说明能力边界并停止，不要暗示 connector 可以 merge、dedupe、tag 或 update
- 没有 Better BibTeX：`library citekey` 仍查原生 `citationKey` 和 Extra。插件没把 key 存进库、插件也不可达时，报告来源限制。前缀和大小写不一致都不是命中
- 没有 Pdfium：不要承诺 fulltext / outline / annotation / PDF 下载后的文本处理
- 没有 embedding：semantic 检索说明会降级；workspace 问答改用 `--mode bm25`
- `attach-mode auto` 没找到 OA PDF：条目仍可能创建成功
