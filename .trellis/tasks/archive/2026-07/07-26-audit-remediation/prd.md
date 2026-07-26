# GPT审计整改:安全与可靠性优化

## Goal

基于 `zotero-cli-code-audit-2026-07-25.md`(GPT Pro 审计,快照 main@dfc672b)的整改父任务。
本父任务持有:审计条目 -> 核实结论 -> 子任务的完整映射、跨子任务验收标准、修复顺序约束。
父任务本身不承担实现,实现全部落在 11 个子任务。

## 核实结论(2026-07-26,dev@39aaeb4,6 个只读核查代理逐条比对)

- 33 条指控:**32 条 CONFIRMED,1 条 PARTIAL**(sidecar 并发:RagIndex 已有 WAL+busy_timeout,
  但全 workspace 共享的 `.md_cache.sqlite` 没有),**无一条 REFUTED/OUTDATED**。
- 已部分修复:lint 继承 1/5(zot-cli 已加 `[lints] workspace=true`,其余 4 crate 缺失)。
- 严重度校准:Debug 凭据泄漏(报告 P2)当前无实际 `{:?}` 调用点,属潜在面;
  doctor `available` 字段 JSON 中已诚实标注 `checked: credentials-only`,属命名误导而非虚假声明;
  config init 静默忽略是代码注释自认的历史行为。
- 报告所有行号基于旧 commit 有 1-50 行漂移,各子任务 description 中的 file:line 已更正为当前代码位置。

## 审计条目 -> 子任务映射

| 子任务 | 优先级 | 覆盖的审计条目 |
|---|---|---|
| 07-26-fix-pdfium-cwd-rce | P0 | P0-01 CWD 动态库劫持(QW-01) |
| 07-26-fix-credential-path-boundary | P1 | API key 跨域上传泄漏(QW-02) + workspace 路径逃逸(QW-03) |
| 07-26-fix-pdfium-download-verify | P1 | Pdfium 下载无校验/非原子安装(M-02) |
| 07-26-fix-sqlite-snapshot | P1 | immutable=1 + 手工 DB/WAL/SHM copy(QW-05/M-01) |
| 07-26-fix-json-error-contract | P1 | --json 错误协议不稳定(QW-04) |
| 07-26-fix-batch-write-gate | P1 | tag batch 无 confirm/失败不留痕(M-03 最小化) |
| 07-26-fix-config-credential | P2 | Debug 泄漏/非原子写/fallback "."/UTF-8 panic/profile null/silent no-op/doctor 语义/config init(QW-06) |
| 07-26-fix-db-semantics-perf | P2 | trash 默认/collection 歧义/N+1/search 全量水合/duplicates 10k+O(N²)/graph clique/async 阻塞(M-04) |
| 07-26-fix-remote-http-hardening | P2 | retry/Retry-After/API-Version/错误体限长/OA SSRF/附件 orphan/arXiv regex(M-05) |
| 07-26-fix-local-boundary-misc | P2 | 附件名逃逸/no-clobber/annotation 校验/graph viewer URL/sidecar 统一/connector TOCTOU(M-06 部分) |
| 07-26-fix-engineering-baseline | P2 | CI matrix/--locked/MSRV/audit/4 crate lints/justfile 漂移/AGENTS.md/CHANGELOG 1.0.0/rmcp(QW-07/M-07) |

## 修复顺序约束(写入各子任务,树结构不表达依赖)

1. `fix-pdfium-cwd-rce`(P0 热修)必须最先,单独可发 security hotfix。
2. `fix-credential-path-boundary` 紧随其后(报告修复顺序 2、3)。
3. `fix-pdfium-download-verify` 依赖 `fix-pdfium-cwd-rce` 落地(候选路径已收敛)。
4. `fix-batch-write-gate` 的错误输出格式依赖 `fix-json-error-contract` 的稳定 error code,建议后者先行。
5. 其余 P2 子任务相互独立,可并行。

## 跨子任务验收标准

- [x] P0/P1 全部关闭:CWD 候选删除、上传请求无 key、workspace 全入口校验、下载有校验、
      immutable=1 移除、--json 全错误路径 envelope、tag batch 有 confirm。
- [x] 安全回归测试落地(报告 8.1 清单为准):doctor 不考虑 CWD、upload server 无 key header、
      workspace name property test、checksum 拒绝篡改、secret canary 不出现在 Debug/错误输出。
- [x] `cargo test --workspace` 全绿;新增测试覆盖每个子任务的核心分支。
- [x] 每个子任务完成后同步对应 .trellis/spec(zot-local/zot-remote/zot-cli backend)。
- [x] 全部完成后 CHANGELOG 记录安全修复,并按报告建议在 release notes 提示曾用附件上传的用户轮换 API key。

## 范围外(报告长期项,不在本轮建任务)

- L-01 Application/use-case 层、L-02 拆分 zot-local god object、完整 MutationPlan/OperationJournal(M-03 全量)、
  L-03 release engineering(SBOM/签名/provenance)、L-04 observability、L-05 MCP 前置条件。
- 待本轮 P0-P2 收敛后另行立项。

## Notes

- 审计报告原文:仓库根 `zotero-cli-code-audit-2026-07-25.md`(未跟踪文件,勿删)。
- 各子任务 description 已内嵌当前代码 file:line 证据与修复要点,子任务规划时以此为起点。
