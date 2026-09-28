# 批准后的实施与验收

日期：2026-09-28。用户明确回复“批准执行”，授权 C1–C4 的本地修改、必要工具准备和验收。修复前事实保留在 [audit-report.md](./audit-report.md)；该审查报告不代表修复后的状态。

验收时状态：C1–C4 的本地实施、检查与写回已完成。C1/C2 通过独立复核；主会话完成全部差异的强模型最终复核，未发现阻塞缺陷。该快照记录提交前的 review 状态。后续提交与归档状态见 [交付记录](./delivery-record.md)。

## 变更边界

基线为 `dev` / `ba4e10e987e7afe65dccb8450445d41bc9e08d79`，初始工作区干净。执行期间未创建提交。证据使用基线 HEAD、文件内容哈希、命令、时间和退出码，不能把基线 HEAD 称为包含修补的新提交。

| 子任务 | 优先级 | 批准范围 | 当前结果 |
| --- | --- | --- | --- |
| C1 security-lock-refresh | P1 | `Cargo.lock`；必要时才允许根 manifest | 锁文件内 4 项必要依赖更新，305 个包无增减，manifest 未改；安全与兼容性门通过 |
| C2 harness-platform-routing | P1 | `workflow_phase.py` 与正式回归测试 | 三组别名修复；10 项新增测试通过，全部 Python 测试 23 项通过；修前有 20 个失败子测试 |
| C3 harness-contract-refresh | P1 | `AGENTS.md`、五工具说明、Codex 项目配置、双语 README | 五工具矩阵与证据边界更新；显式 Codex inline；README 修正为 Web API 合并、connector 仅导入 |
| C4 evergreen-security-cadence | P2 | CI schedule、五工具说明与现有质量规范 | 周一 08:17 UTC 仅 security；既有 push/PR/manual 保留全部 8 个展开作业；security 权限限 contents: read |

## 已通过的本地验证

锁文件 SHA-256：`857974bce8e060ed25212b68463e2bd1e0c42be3dce908517d001428d93e706f`。

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| `cargo audit` 0.22.2 | exit 0 | `research/c1-cargo-audit-result.json` |
| `cargo deny check` 0.20.2 | exit 0；保留已有 duplicate 警告 | `research/c1-cargo-deny-result.json` |
| `just ci`（C1 后） | exit 0；281 个不同 Rust 测试，原有 Python 测试 13 项 | `research/exec-just-ci.result.json` |
| `cargo +1.85.0 check --workspace --locked` | exit 0 | `research/exec-msrv.result.json` |
| `cargo machete` 0.9.2 | exit 0 | `research/exec-machete.result.json` |
| `cargo +nightly-2026-07-01 udeps --workspace --all-targets --locked`，udeps 0.1.61 | exit 0 | `research/exec-udeps.result.json` |
| `just ci`（全部修改后） | exit 0；281 个不同 Rust 测试、23 项 Python 测试 | `research/final-just-ci.result.json` |
| `npm --prefix docs run build` | exit 0；VitePress 1.6.4，现有 Node 26.7.0 环境 | `research/final-docs-build.result.json` |
| actionlint 1.7.12 | exit 0 | `research/exec-actionlint.result.json` |
| 四类 CI 事件集合与基线结构比较 | exit 0；8/8/8/1 个展开作业 | `research/c4-event-matrix.json` |
| 最终差异、任务关系与上下文清单 | 通过 | `research/final-provenance.json`、`research/final-task-validation.json` |

版本守卫在 `just ci` 中执行两次，因此 283 次 Rust 测试执行对应 281 个不同测试。完整依赖必要性与 RustSec 数据库提交见 [C1 复核](./research/c1-implementation-review.md)。

最终 `just ci` 与文档构建的产品输入哈希在检查前后相同，检查没有改写源文件。复用的 C1 安全、MSRV 和依赖分析结果均绑定当前锁文件。Hosted docs 使用 Node 20；本次本地构建没有替代 Node 20 或跨平台 hosted 验证。

## 验收映射

| 需求 / AC | 完成证据 |
| --- | --- |
| R1 / AC1；C1-AC1–3 | 安全补丁、锁闭包复核、audit/deny、stable/MSRV、machete/udeps 全部通过；未新增 ignore 或业务绕过 |
| R2 / AC2；C2-AC1–3 | 三组别名修复；修前 20 个失败子测试，修后全部 23 项 Python 通过；实际五平台两步骤正文验证通过 |
| R3–R4 / AC3；C3-AC1–4 | 配置实际解析为 codex-inline；根 CLAUDE.md 保持 @AGENTS.md；五工具规则、发现、权限与模型边界写回；双语后端承诺与 WebMergeWriter 一致 |
| R5 / AC4；C4-AC1–4 | 四事件矩阵、actionlint、权限/依赖/失败传播复核通过；共享说明和质量规范写回；真实 schedule 保持 UNVERIFIED |
| R6 / AC5 | 修复前报告与执行结果分开；记录 HEAD、内容哈希、UTC 时刻、命令、退出码；环境握手错误原因未查明 |
| R7 / AC6 | 正式说明标注五工具共享与 Codex-only；[最终审查](./research/final-review.md)覆盖全部差异与证据限制 |
| AC7 | 保留一个父任务和四个子任务；批准前 planning，批准后按进展启动；最终 review，父子关系与清单验证通过 |

完整差异为 9 个已有受管文件和 1 个新增回归测试，另有本次 Trellis 任务树。业务 Rust 源码、根 Cargo.toml、canonical skills、CLAUDE.md、Pages 工作流和模板哈希未改。

## 环境问题

Cargo 查询与固定工具安装曾因 crates.io `config.json` 的 schannel TLS 握手错误失败。PowerShell 对同一 HTTPS 地址返回 200；握手错误的原因未查明。TLS 校验保持启用，Cargo 网络配置未修改。

锁更新通过现有缓存精确离线解析完成，随后 audit/deny 在线刷新公告并通过。machete/udeps 改用官方仓库固定版本 Windows 发布资产，按 GitHub release API 的 SHA-256 digest 校验后放入被忽略的 `target/audit-tools/`。来源、版本和摘要见 `research/exec-tool-downloads.json`。两个工具均在进程级 PATH 中运行。Rust 1.85.0 与固定 nightly 已准备，默认工具链未改变。

## 审查与模型分工

C1/C2 由独立 `trellis-check` 代理复核，未发现阻塞缺陷；独立重跑 23 项 Python 测试通过，Rust 检查采用主会话同一锁文件的日志。见 [独立复核](./research/c12-independent-review.md)。

本轮分派发生在项目 Codex 配置仍为 auto 时，使用继承的主模型；未配置低价模型或声称自动降档。批准的 inline 配置生效后，后续实施与检查在主会话执行。面向低成本执行者的文件范围、测试和升级条件写入五工具说明；安全、权限、根因和最终验收保留强模型审查。

## 未验证与未执行

五套客户端原生新会话、hook 注入、实际模型切换、OS 隔离、真实 Zotero Web/connector 写入、PDF/embedding 操作未验证。修补后的 Linux/macOS hosted CI、新提交 SHA 和真实 scheduled run 未产生。旧提交的历史绿色状态不能替代这些验证。

未提交、推送、创建 PR/Issue、归档任务或发布。没有修改全局 harness 配置、canonical skills、Basic Memory 或团队知识库。批准结论选择回写仓库说明与现有 spec，并标明五工具共享或 Codex 专属。
