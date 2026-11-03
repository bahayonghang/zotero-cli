# 最新结论（覆盖下方历史未授权状态）

2026-11-02 核心实现与验收完成，进入 review 待提交收口。Windows Zotero 10.0.7 独立 profile/data：标签增删、根/子分类创建、3 条不同决策批量 apply、API/UI/正常重启回读通过；原实例已恢复。173 个 CLI 测试及 workspace/质量门通过。故障注入与真实桌面证据分列；用户要求聚焦核心，不扩展桌面故障测试。未 commit/push/PR，未归档。见父 validation.md。

真实证据：父任务 acceptance/run.py 与 ignored target/local-organization-acceptance/2026-11-02 下的场景 JSON、plan.json、report.json、restart.json；父 validation.md 汇总 AC 与边界。

# Validation — 本地单条标签

日期：2026-11-02。分支 `dev`，基线 `eb0345f`。未 commit、未 push、未创建 PR。
调用路径：仓库未安装 `zot`，本轮固定 `cargo run -q -p zot-cli -- --json`。
本轮没有启动 Zotero，没有创建或切换 profile，没有对主库或任何真实库授权、写入或重启。

## 本轮 doctor

`cargo run -q -p zot-cli -- --json doctor` 成功，且未试写：

- `local_sqlite_read.available=true`，`db_exists=true`。
- `local_api_write.reachable=false`，错误 `local-api-unreachable`；`supported=false` 只表示当前探测不到，不能推断桌面版本不支持。
- `credential_present=false`，`verified=false`。
- `web_write.configured=false`。
- connector 与 local HTTP read 不可达。

这是环境快照，不是真实验收结论。

## 自动化检查

| 检查 | 结果 |
| --- | --- |
| `cargo test -p zot-cli local_write --locked` | PASS，20 个 local_write 测试 |
| `cargo test -p zot-desktop --locked` | PASS，20 个测试 |
| `cargo test -p zot-cli --test json_error_contract --locked` | PASS，6 个测试 |
| `just skills-sync` 后 `just ci` | PASS：fmt、locked check、clippy、全部 workspace 测试、skill 镜像与 23 个 Python 测试 |
| `npm --prefix docs run build` | PASS |
| `git diff --check` | PASS；仅有工作副本 LF/CRLF 提示，无空白错误 |

`just ci` 不改写文件。canonical skill 有意修改后才运行 `just skills-sync`；检查确认 `.agents/skills/zot` 与 `.claude/skills/zot` 和 canonical 一致。

## AC 映射

证据级别：mock = 假 HTTP/纯函数；真实 CLI = 本机 `zot` 进程；真实桌面 = 隔离 Zotero profile 与 data 的授权、写入、UI 和重启。mock 故障不是桌面证据。

| AC | mock | 真实 CLI | 真实桌面 | 结论 |
| --- | --- | --- | --- | --- |
| T-AC1 中文、空格、特殊字符可预览、确认、回读，新标签为手工类型 | PASS | 未覆盖成功写入 | 未授权，未做 | 部分完成 |
| T-AC2 保留 type 1、type 0/省略和未删除对象；顺序不同不失败 | PASS | 未覆盖成功回读 | 未授权，未做 | 部分完成 |
| T-AC3 重复添加、移除不存在标签为 unchanged，不读凭据、不授权、不 PATCH | PASS | 未对真实 API 跑 unchanged | 未授权，未做 | 部分完成 |
| T-AC4 空白、非法 key、子条目、回收站在提交前拒绝；冲突/401/未知/回读失败沿用契约 | PASS | 空白与 Web 确认参数在真实进程拒绝 | 未授权，未做 | 部分完成 |
| T-AC5 Web 默认与旧 batch 兼容；local 路径不调用 SQLite 或 Web | PASS（结构 + clap + 既有 batch 测试） | Web 确认参数在联网前拒绝 | 未授权，未做 | 部分完成 |
| T-AC6 隔离库新增、移除、重复、UI 确认与重启回读；测试、文档、skill | 测试、文档、skill 镜像 PASS | 仅错误契约进程测试 | 未授权，未做 | 不能标完成 |

### T-AC1

- mock：`tag_diff_preserves_types_duplicates_and_rejects_blank_or_illegal_input` 覆盖 `待读`、`C++/安全`、前导空格和 `Priority`，新对象为 `{"tag","type":0}`。
- mock：`confirmed_tag_write_patches_complete_array_once_and_accepts_reordered_readback` 对 `待读` 与 `C++/安全` 做一次 PATCH，回读省略 type 0 且顺序不同，结果 `applied` + `verified=true`。
- 真实 CLI / 桌面：Local API 不可达，且本轮未授权真实写入。没有成功预览或确认证据。

### T-AC2

- mock：差异测试保留 type 1 的 `meta`、type 0、省略 type，以及同名 `重复` 的两个对象。
- mock：`tag_readback_ignores_order_and_manual_type_shape_but_keeps_duplicates` 证明顺序和嵌套属性顺序不影响比较；type 1 与省略不等价；重复数量和未知属性缺失会失败。
- 真实桌面未做，不能证明 Zotero UI 或重启后的类型。

### T-AC3

- mock：已有精确名称添加、移除不存在名称返回原数组；`tag_preview_and_noop_never_authorize_or_patch` 对预览和 unchanged 使用损坏凭据文件，请求只有 GET，没有授权或 PATCH。
- 真实桌面未做。

### T-AC4

- mock：纯空白、空输入、非法服务端 tags（非数组、缺名称、type 2、字符串 type）在规划阶段拒绝，不以空数组代替。
- mock：`tag_rejects_invalid_targets_and_illegal_tags_before_patch` 在 PATCH 前拒绝子条目、回收站、note、非法 key、非法 tags 和过期 version；请求没有 PATCH。
- mock：`expired_tag_key_reauthorizes_once` 只在明确 401 后重新授权一次。`tag_unknown_and_verification_failure_are_not_success` 覆盖 500 后的 `unknown` 与接受后回读不符的 `verification-failed`。共享 `execute` 测试覆盖 403 不重试和超时未知回执；标签路径调用同一 `execute`，但没有单独的标签形态 403/超时夹具。
- 真实 CLI：`json_error_contract::local_tag_validation_runs_in_real_process_without_network` 证明 add/remove 的 Web 确认参数返回 `local-write-options`，空白和缺少 `--tag` 返回 `local-write-tag`。stdout 是单个 JSON envelope，exit 1，stderr 为空。既有 item dispatcher boxing 未移除。
- 真实桌面的拒绝授权、只读组库和提交超时未做。

### T-AC5

- mock：不带 `--write-backend` 时解析为 Web；Web 确认参数由 `validate_web_options` 拒绝。`item tag batch --write-backend local` 解析失败。既有 batch 测试仍通过，batch 参数结构未加 local 字段。
- local 分支在 `apply_tag_update` 中直接返回，不调用 `ctx.remote()` 或 `ctx.local_library()`。这是代码结构证据，不是会在误开 SQLite 时失败的运行时哨兵。
- 真实 CLI 只证明错误路径在联网前停止。成功 local 预览不打开 SQLite 尚未用不可达 SQLite 或桌面对照证明。

### T-AC6

自动化测试、双语文档、canonical skill、evals/test-prompts 和镜像检查已完成。文档只写单条本地标签，没有把分类创建或批量整理写成已支持。

隔离库新增、移除、重复执行、授权框、UI 类型和重启回读没有授权，未执行。因此本子任务不能标全部完成，父任务 AC1/AC7 也不能据此标完成。

## 未覆盖环境

- 真实 Zotero 10+ Local API、组库、其他操作系统。
- 用户授权框 Deny / Allow / Always Allow。
- 提交超时、进程中断和重启后的标签回读。
- 成功路径是否打开 SQLite 的运行时反证。

## Codex 接续更新（2026-11-02）

上述为 Grok 首轮记录。接续已通过最终 just ci、文档构建、diff 检查；B 另补矛盾/畸形创建回执回归并修复。用户已授权隔离桌面验收，独立 profile/data 和三条合成文献已建立，当前等待用户手动启用测试实例 Local API；尚未通过真实写入/UI/重启验收。最新环境、证据与恢复要求以父任务 validation.md 为准。
