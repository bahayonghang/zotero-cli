# C1 / C2 独立实施审查

日期：2026-09-28。审查角色：独立 `trellis-check` 子代理，继承主会话强模型配置，未指定低成本模型。适用工具：Claude Code、Codex、Grok Build、Kimi Code、OMP。

## 结论

C1 / C2 当前代码差异通过审查，没有发现需要修复的阻塞缺陷。本审查未改产品代码、任务状态或全局配置。审查产物包括本报告、`c12-review-checks.json`、`c12-lock-scope.json` 和对应日志。

C1 必需的本地检查已由主会话或 C1 执行代理通过。本审查读取原始日志、结果 JSON 和当前锁文件，未重复 Rust 编译。C2 的 23 项 Python 测试由本审查独立重跑通过。主会话仍需在 C3 / C4 集成完成后运行包含新增测试的最终 `just ci`。

本结论仅覆盖批准的 C1 / C2 工作树内容。未验证修补后的跨平台 hosted CI、定时任务实际运行、五套 harness 的新会话、hook、权限隔离或原生子代理。

## 审查范围与依据

- 读取 C1 / C2 的 `prd.md`、`design.md`、`implement.md`、`check.jsonl` 及引用材料。
- 读取完整 SubagentStart hook 输出、`trellis-check` skill、各现有 package spec index、依赖质量规范及代码复用指南。
- 对照 `Cargo.toml`、`deny.toml`、`justfile`、当前 `Cargo.lock` 与基线锁文件。
- 追踪 `get_context.py` → `common.git_context` → `resolve_effective_platform` / `filter_platform` / `_platform_matches`，核对真实 `workflow.md` 的 2.1 / 2.2 段落。
- 基线提交：`ba4e10e987e7afe65dccb8450445d41bc9e08d79`。本次没有创建提交。锁文件和测试文件内容哈希见 `c12-review-checks.json`。

## C1：安全锁文件

当前 `Cargo.lock` 的 SHA-256 为 `857974bce8e060ed25212b68463e2bd1e0c42be3dce908517d001428d93e706f`，与 audit、deny、stable、MSRV、machete、udeps 结果记录一致。基线与当前锁文件均含 305 个包，没有新增或删除包。仅以下四个包改变：

| 包 | 原版本 | 当前版本 | 必要性与审查结果 |
| --- | --- | --- | --- |
| rustls | 0.23.38 | 0.23.45 | 精确安全补丁；与已保存公告的 patched 范围一致 |
| aws-lc-rs | 1.16.2 | 1.18.1 | rustls 0.23.45 要求 `^1.18` |
| aws-lc-sys | 0.39.1 | 0.45.0 | aws-lc-rs 1.18.1 要求 `^0.45.0` |
| rustls-webpki | 0.103.13 | 0.103.15 | rustls 0.23.45 要求 `^0.103.14` |

独立读取本地 registry 缓存，核对四个版本的 checksum、未撤回状态、依赖约束及 Rust 版本声明。缓存元数据与 C1 证据一致；锁文件 checksum 一致。四个包声明 Rust 1.71 / 1.71.0，实际 Rust 1.85.0 locked workspace check 也通过。具体字段见 `c12-lock-scope.json`。

`aws-lc-sys` 跨越 0.x 次版本，是上层已选 TLS 补丁要求的传递依赖变动；本仓库未新增直接依赖或修改 TLS provider。新增构建依赖边指向锁中已有的 `pkg-config`，仅适用于 Unix。Windows 本地编译通过；Linux / macOS 新锁构建仍需后续 hosted CI。

`Cargo.toml` 和 `deny.toml` 无差异，`deny.toml` 保持 `ignore = []`。没有业务源码改动、TLS 验证绕过或无关依赖升级。锁文件更新命令为精确的 `cargo update -p rustls --precise 0.23.45 --offline`；在线 audit 随后刷新公告并通过。

C1-AC1、C1-AC2 和 C1-AC3 的本地代码与检查要求有证据支持。计划中的“新 SHA”追踪目前以基线提交加内容哈希实现；没有获准创建提交，不能把基线 SHA 称为修补后的提交。后续授权提交时应补记实际新 commit SHA。

## C2：平台路由

统一匹配边界 `.trellis/scripts/common/workflow_phase.py:135` 仅添加三组已确认别名：`claude` → `claudecode`、`kimi` → `kimicode`、`omp` → `ohmypi`。输入和 marker 名称均先保留原有大小写、空格、横线、下划线归一化，再应用别名。

没有把 OMP 与 Pi 合并，没有把 Claude Desktop 与 Claude Code 合并，也没有扩大未知平台匹配。`resolve_effective_platform` 未修改，Codex 的默认/auto、显式 inline、legacy sub-agent、非法配置回退语义保持原状。没有修改 workflow 授权文本、ignored 原生适配器或 template hash 基线。

| 验收条款 | 代码与证据 | 结果 |
| --- | --- | --- |
| C2-AC1：三组别名及五平台 2.1 / 2.2 正文 | `scripts/tests/test_harness_platform_routing.py:23`、`:139`、`:163`、`:169` | 断言 agent 类型、任务路径、实施/审查条款；没有仅比较两个空输出 |
| C2-AC2：Codex 模式与其他平台保持 | 同文件 `:35`、`:55`、`:79`、`:98`、`:181`、`:197` | 覆盖 auto/default、inline、sub-agent、invalid；覆盖 Grok、Cursor、Antigravity、Pi、未知名称 |
| C2-AC3：先红后绿与测试隔离 | `c2-tests-red.log`、`c2-tests-green.log`、`c12-python-tests.log` | 修前 10 个测试含 20 个失败子例；修后 10 个路由测试通过；全量 23 个 Python 测试通过 |

CLI 用例调用实际 `get_context.py`，复制当前 workflow 到临时 `.trellis` 目录，写入临时配置。根目录查找以 cwd 向上寻找 `.trellis`，因此临时配置可控制真实解析路径。测试不调用 `task.py start`、不写用户配置、不启动 harness 模型。当前仓库配置用例另行覆盖实际工作树模式。

现有 `just skills-check` 使用 `unittest discover -s scripts/tests -p test_*.py`，新测试已进入正式发现范围。主会话先前的 `exec-just-ci.log` 仅含旧的 13 项 Python 测试；不能以该次结果单独声称新增用例已纳入完整 gate。独立的 23 项运行已通过，最终集成 gate 由主会话补齐。

## 验证记录

| 检查 | 结果 | 来源 |
| --- | --- | --- |
| Rust fmt / clippy | 通过 | 读取主会话 `exec-just-ci.log` / `.result.json`，exit 0 |
| Rust stable locked check / workspace tests | 通过 | 同上，未重复编译 |
| Rust 1.85.0 locked check | 通过 | `exec-msrv.log` / `.result.json`，锁哈希一致 |
| cargo audit / cargo deny check | 通过 | `c1-cargo-audit-*`、`c1-cargo-deny-*`；deny 保留既有 duplicate 警告 |
| cargo-machete 0.9.2 | 通过 | `exec-machete.log` / `.result.json` |
| cargo-udeps 0.1.61 / nightly-2026-07-01 | 通过 | `exec-udeps.log` / `.result.json` |
| Python 全量测试 | 通过，23 项 | 本审查执行，2026-09-28 06:43 UTC；`c12-python-tests.log` |
| scoped git diff --check | 通过 | 本审查执行；`c12-diff-check.log` |
| C3 / C4 集成后的最终 just ci | 待主会话执行 | 本审查未扩大到后续实施范围 |

Python 路由代码没有独立配置的静态类型检查 gate；已通过实际 CLI 与 unittest 执行。没有新增忽略项或降低检查要求。

## 回写与未验证项

C3 应记录这三组平台 ID/显示名、实际步骤渲染命令、Codex-only 显式 inline 决策及 Trellis 升级后的回归要求。C4 应在共享质量规范中记录安全扫描节奏和新公告使历史绿色结果过期的边界。上述回写已有独立批准子任务，避免在本审查中重复改写相同文件。

五套工具的新会话与 native 行为、Linux/macOS hosted 编译、定时扫描的实际触发均未验证。现有证据不能扩展为这些检查的通过结论。
