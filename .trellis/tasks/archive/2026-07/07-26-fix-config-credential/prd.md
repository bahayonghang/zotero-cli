# P2: 配置与凭据加固

## Goal

修复审计报告中已确认的配置与凭据安全、持久化可靠性和运行时配置契约问题，使 secret 不会经 Debug 泄漏、配置写入具备原子性且不回退到当前目录，并让 profile、输出默认值和 doctor/config init 的机器可读语义与真实行为一致。

## Background

- 父任务映射：`.trellis/tasks/07-26-audit-remediation/prd.md:29` 的 `07-26-fix-config-credential`。
- 源证据：`zotero-cli-code-audit-2026-07-25.md:131-136`、`:158`、`:166`。
- `AppConfig`、`ZoteroConfig`、`ProfileConfig` 和 `EmbeddingConfig` 当前派生 `Debug`，凭据字段是裸 `String`；`AppContext::Debug` 又输出完整配置（`src/zot-core/src/config.rs:35-129`，`src/zot-cli/src/context.rs:26-35`）。
- `AppConfig::save` 直接覆盖目标文件，Unix 权限在写入后才收紧；配置目录缺失时回退 `.`（`src/zot-core/src/config.rs:132-185`）。
- `redact_secret` 按字节切片，非 ASCII secret 可能在 UTF-8 边界 panic（`src/zot-core/src/config.rs:330-335`）。
- `AppConfig` 已提供 `effective_profile_name`，但 `AppContext` 只记录显式 `--profile`，默认 profile 的 envelope 元数据为 `null`（`src/zot-core/src/config.rs:235-252`，`src/zot-cli/src/context.rs:39-50`）。
- `output.default_format` 和 `output.limit` 自 0.2.0 起即为公开配置并可由 `config set` 修改，但业务命令未消费；直接删除会破坏已发布契约。
- `doctor` 的 Web 写能力把“凭据已配置”同时标成 `available=true`，而实际未验证 key、scope 或权限（`src/zot-cli/src/commands/doctor.rs:210-221`）。
- `config init --target-profile` 会静默忽略 `embedding-*` 根级设置，而 `config set` 对相同行为会报错（`src/zot-cli/src/commands/config.rs:259-278`）。

## Requirements

### R1: Secret 类型与 Debug 边界

- 在 `zot-core` 中以 serde-transparent 的 secret 值类型封装 Zotero、Semantic Scholar 和 embedding API key。
- secret 类型及包含它的配置结构的 `Debug` 输出必须只显示 redacted 状态，不能包含完整 secret、前缀或后缀。
- `AppContext::Debug` 可以保留非敏感诊断字段，但不得绕过配置层的 redaction。

### R2: 原子且受限的配置持久化

- 配置必须写入目标文件同目录的临时文件；在写入 secret 前即使用受限权限，完成 write/flush/sync 后原子替换目标。
- Unix 目标文件最终权限必须为 `0600`，并同步父目录元数据；Windows 使用原子替换并继承用户配置目录 ACL，不声称已验证 ACL。
- 序列化、临时写入或替换失败时不得先截断现有配置。
- 复用 workspace 已有 `tempfile` 依赖，不引入新的外部包。

### R3: Secret 配置路径 fail closed

- `config.toml` 的 load/save/show/doctor 路径只能来自系统配置目录；系统目录不可用时返回结构化 `config-dir-unavailable` 错误。
- 不得将 secret 配置回退到当前工作目录。
- 非 secret 的 cache/workspace/index state path 不在本要求内；其现有 API 可使用明确命名的 state-dir fallback，不能重新作为 config 文件路径。

### R4: Unicode 安全 redaction

- `redact_secret` 按 Unicode scalar value 处理，短 secret 返回 `(set)`，较长值仅显示最后四个字符。
- ASCII 现有显示契约保持兼容，非 ASCII 输入不得 panic。

### R5: 有效 profile 元数据

- 显式 `--profile` 优先于默认 profile；未显式指定时，实际 materialize 的默认 profile 名必须写入成功和运行期错误 JSON envelope 的 `meta.profile`。
- 若配置加载前即发生 CLI 解析错误，只能使用显式命令行 profile，不虚构默认 profile。

### R6: 输出配置真正生效

- `output.default_format` 仅允许 `table` 或 `json`；`--json` 显式开启优先，配置的 `json` 可作为缺省输出协议。
- 配置 JSON 默认值必须同时作用于成功输出、运行期错误和不支持 JSON 的长驻/原始输出协议检查。
- `output.limit` 必须是正整数，只作为未显式提供 `--limit` 时的只读结果集默认值。
- 适用命令：library search/list/recent/feed-items/semantic-search/duplicates，item related/deleted/note search，collection search，workspace show/query，annotation list/search，Scite search/retractions。
- 不适用：semantic index、dedupe/merge、tag batch、sync update-status 等索引工作量或写入安全上限；它们保持各自命令默认值。

### R7: Doctor Web 写能力语义

- Web 写能力 JSON 必须区分 `configured` 与 `verified`。
- 本任务不新增联网验证；因此 `verified=false`、`permissions=null`，并以 `last_error=null` 表示未执行验证。
- 删除容易误解的 `available` 字段，保留 `checked=credentials-only` 和缺失凭据 hint。

### R8: Config init fail-fast

- `config init --target-profile` 收到 `embedding-url`、`embedding-key` 或 `embedding-model` 等根级限定参数时，必须返回与 `config set` 一致的 `config-key` 错误。
- 失败路径不得保存部分配置或把 `--make-default` 落盘。

## Acceptance Criteria

- [x] AC1: secret canary 在 `SecretString`、各配置结构和 `AppContext` 的 Debug/错误输出中均不可见；TOML round-trip 仍保持原值。
- [x] AC2: 原子 save 覆盖已有配置成功，无残留临时文件；Unix 文件 mode 为 `0600`，失败路径不预先破坏目标。
- [x] AC3: 缺少系统配置目录的测试 helper 返回 `config-dir-unavailable`，且路径不含 `.`/CWD fallback。
- [x] AC4: `redact_secret` 对 ASCII、中文和 emoji 边界均不 panic，并符合四字符尾部契约。
- [x] AC5: 显式与默认 profile 的优先级正确，成功/运行期错误 envelope 均携带实际 profile。
- [x] AC6: `output.default_format=json` 在未传 `--json` 时产生 JSON，并对 `graph serve`/`completions` 执行协议拒绝；显式 `--json` 行为不回归。
- [x] AC7: 配置 limit 只补全白名单只读命令；显式 `--limit` 优先，写入和索引命令默认值不变。
- [x] AC8: doctor Web 写能力不再输出 `available`，并稳定输出 `configured/verified/permissions/last_error/checked`。
- [x] AC9: profile init 携带 root-only key 时 fail-fast，配置对象和默认 profile 均不发生部分变更。
- [x] AC10: `cargo test -p zot-core`、配置/CLI/doctor 聚焦测试和 `just ci` 全部通过。

## Out Of Scope

- 不引入 OS keychain、凭据迁移或 config 文件加密。
- 不在 doctor 中发起远程 key/permission 验证。
- 不为 Windows 自建 ACL 管理器；只诚实报告本任务没有验证 ACL。
- 不改变索引、批量写或同步命令的安全/工作量上限。
- 不处理父任务列出的长期 MutationPlan、resume/reconcile 或其他 P2 子任务。
