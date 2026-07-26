# 父任务集成验收

验收日期：2026-07-26

源证据：`zotero-cli-code-audit-2026-07-25.md`，审计快照
`main@dfc672bfcea344260c03abbcb5ef213edd116407`。本轮以当前 `dev` 的实际代码、测试、
归档任务和本地门禁为准，替代源报告中“未实际执行”的动态验证缺口。

## 1. 审计映射闭环

| 子任务 / 审计项 | 实现提交 | 集成证据 | 结论 |
|---|---|---|---|
| `fix-pdfium-cwd-rce` / P0-01、QW-01 | `982c012`, `c1e2d96` | `candidate_library_paths_only_uses_trusted_sources`；删除 CWD 与裸库名 fallback | PASS |
| `fix-credential-path-boundary` / QW-02、QW-03 | `1f31505`, `242e99d` | `external_attachment_upload_never_receives_zotero_api_key`；`WorkspaceName` 全入口校验与 symlink containment tests | PASS |
| `fix-pdfium-download-verify` / M-02 | `6b65241` | 固定平台 checksum、大小上限、临时文件与原子发布；篡改、截断、错平台、并发安装 tests | PASS |
| `fix-sqlite-snapshot` / QW-05、M-01 | `b1b7f50` | SQLite Backup API；WAL、一致性 invariant、busy error 与 snapshot metadata tests | PASS |
| `fix-json-error-contract` / QW-04 | `f4810f7` | `AppError` 统一 runtime/parse/protocol envelope；`json_error_contract` integration tests | PASS |
| `fix-batch-write-gate` / M-03 最小范围 | `33808ae` | batch preview、`--confirm`、`--max-affected`、逐操作 partial-state 与继续执行 tests | PASS |
| `fix-config-credential` / QW-06 | `472d1da` | secret-safe Debug、Unicode redaction、原子 config write、effective profile/output、doctor capability tests | PASS |
| `fix-db-semantics-perf` / M-04 | `1bfeaae` | trash policy、collection 歧义、batch hydration、candidate/edge budgets、blocking-thread tests | PASS |
| `fix-remote-http-hardening` / M-05 | `11184bc` | eligible retry/Retry-After、API version、bounded error、OA redirect policy、orphan cleanup、arXiv XML tests | PASS |
| `fix-local-boundary-misc` / M-06 选定范围 | `8ae247f` | attachment basename/no-clobber、annotation rectangle、graph CSP/URL、sidecar schema/WAL、connector editability tests | PASS |
| `fix-engineering-baseline` / QW-07、M-07 | `2e176b7` | 三 OS CI、Rust 1.85、locked gates、audit/deny/machete/udeps、5 crate lint guard、1.0.0 CHANGELOG | PASS |

11 个子任务均位于 `.trellis/tasks/archive/2026-07/`，`task.json.status=completed`；
所有现有 `prd.md`/`implement.md` 合计无未勾选项。父任务 `task.py list` 为 `11/11 done`。

## 2. P0/P1 与安全回归

| 源报告 8.1 验收项 | 当前测试证据 | 结论 |
|---|---|---|
| doctor 不考虑当前目录 Pdfium | `pdf::tests::candidate_library_paths_only_uses_trusted_sources` | PASS |
| 外部上传 server 不收到 Zotero API key | `zotero::tests::external_attachment_upload_never_receives_zotero_api_key` | PASS |
| workspace 名称拒绝路径逃逸 | `workspace_name_accepts_only_kebab_case`、`save_rejects_unvalidated_workspace_name`、`load_rejects_workspace_symlink_outside_root` | PASS |
| managed Pdfium 拒绝错误 SHA-256 | `archive_shape_and_library_checksum_fail_closed`、`managed_cache_rejects_legacy_and_tampered_libraries` | PASS |
| managed Pdfium 并发安装保持原子 | `concurrent_installers_download_once`、`existing_verified_library_survives_failed_redownload_attempt` | PASS |
| attachment filename 拒绝 separator/traversal | `rejects_untrusted_attachment_filenames_cross_platform`、`force_never_truncates_the_source_attachment` | PASS |
| OA 下载拒绝 private redirect | `download::tests::rejects_private_redirect_before_second_request` | PASS |
| graph viewer 拒绝非 HTTP(S) URL | `embedded_graph_script_uses_dom_and_http_url_policy`、`every_graph_route_has_browser_security_headers` | PASS |
| config/context Debug 不包含 secret canary | `secret_debug_is_redacted_and_toml_round_trips`、`context_debug_never_exposes_secret_canary` | PASS |

P0/P1 的运行时闭环同时由以下行为保证：本地读取不直接写 `zotero.sqlite`；远程写入仍经
Zotero Web API；Pdfium 下载仅从已归档 P0 建立的可信候选路径进入；batch gate 消费统一的
JSON error code，不反转既定任务顺序。

## 3. 跨任务契约

- [x] P0/P1 全部关闭：CWD/裸库名候选、跨域 key、workspace 逃逸、未校验下载、
      `immutable=1`/手工 sidecar copy、非 envelope JSON 错误、无确认 batch write 均已移除。
- [x] 安全回归清单逐项映射到当前测试，关键失败路径均 fail closed。
- [x] 可复用契约已落入 `zot-cli`、`zot-core`、`zot-local`、`zot-remote` 的 backend specs，
      包含错误、数据库、质量、connector、merge/dedupe 与远程边界。
- [x] `CHANGELOG.md` 的 1.0.0 Security/Reliability/Migration 记录整改，并明确提示曾使用
      附件上传的用户轮换 Zotero API key。
- [x] 父任务最终 `just ci`、Trellis validate、diff 检查与源报告持久化完成。

## 4. 工程门禁证据

父提交前已验证：

```text
cargo +1.85.0 check --workspace --locked
cargo test -p zot-local --lib --locked pdf::tests::
cargo audit --json
cargo deny check
cargo machete
cargo +nightly-2026-07-01 udeps --workspace --all-targets --locked
zot --json doctor
just ci
```

其中 audit 为 0 vulnerability/unsound finding；deny 的 advisories/bans/licenses/sources
全部通过；machete/udeps 均无未使用依赖；`just ci` 已用 dirty-tree 前后指纹证明不改写仓库。

## 5. 范围外复核

未新增 application/use-case layer、完整 MutationPlan/OperationJournal、模块拆分、SBOM、签名、
provenance、自动发布、长期 observability 或 MCP 实现。`rmcp` 在 MCP 实现前保持未声明；
长期架构项仍由父 PRD 的范围外约束持有，本轮未新建任务或扩大产品语义。
