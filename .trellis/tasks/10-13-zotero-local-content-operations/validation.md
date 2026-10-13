# Validation — 2026-10-13

用户已明确批准实施，并在自动化验证通过后确认提交。未推送、未全局安装。任务保留
`in_progress`，因为 AC9 要求的独立测试库真实授权/写入/UI 验收尚未完成。

## 自动化结果

| 检查 | 结果 |
| --- | --- |
| `cargo check -p zot-cli --locked` | PASS |
| `cargo test -p zot-desktop -p zot-cli -p zot-core --locked` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `just skills-sync` | 已有意更新四个本地镜像，与 canonical 一致 |
| 最终 `just ci` | PASS：fmt、locked check、clippy、303 个 Rust 测试、23 个 Python 测试、版本及镜像检查 |
| `npm --prefix docs run build` | PASS，双语 VitePress 页面构建成功 |
| `git diff --check` | PASS |

最后完整检查在超时和作者回读顺序测试补齐后运行，不以先前检查覆盖新改动。
新增 skill evals 是评估场景，不声称运行过模型评估器。

## 真实 Zotero 10.0.6：只读验证

开发二进制：`target/debug/zot.exe`。会话开始使用已安装 `zot` 诊断；新二进制
仅用于验证新代码，未替换全局 `zot`。

- 新版 doctor：Local API reachable/supported=true，API 3/schema 44；
  credential_present=false、verified=false；Web configured=false。
- 同次 doctor：SQLite 仍报 `zotero-db-busy`。后续 Local API 预览成功，证明
  该路径不依赖 SQLite snapshot 或 Web key。
- 对一个从 API 实际枚举的普通条目：缺失 DOI 的补充预览正确返回
  before=null/after=预览值；未发送 confirm，该示例值没有写入。
- 已有标题默认保留，返回 unchanged 和 skipped_fields=[title]。
- 向已有分类添加成员返回 unchanged。
- 从现有分类到另一现有分类的 move 预览正确展示前后成员及分类名称。
- 以上结果都为 write_accepted=false / verified=false。没有请求授权，
  没有操作 Allow/Always Allow，也没有 PATCH 真实库。

## 栈溢出修复证据

Windows debug 可执行文件的 local item update 发生 stack overflow，而测试
线程中的函数调用通过、collection 分支可运行。临时测量发现分发 future
约 68 KB，Local API update future 约 3 KB；深层 async 嵌套放大了主线程栈占用。
临时诊断测试已删除。

- 回归测试：`json_error_contract::local_update_validation_runs_in_real_process_without_network`。
- 修复前运行失败，进程码 -1073741571（Windows stack overflow）。
- `commands::dispatch` 中 box item future 后输入校验通过，但真实 HTTP 预览
  仍溢出；同时 box main 中 run future 后真实预览、doctor 均通过。
- 扫描 main/run/dispatch/item/update/collection 同形调用边界。只在两个
  放大栈占用的入口做堆分配，没有逐个重写所有命令或增加全局栈大小。
- 最终完整 CI 的真实进程回归通过。契约记录在 backend/local-api.md。

## 验收映射与剩余验证

| AC | 当前证据 | 尚未验证 |
| --- | --- | --- |
| AC1 | mock 版本/协议 gate；真实无 Web key 预览 | 真实授权后写入 |
| AC2 | A/C→B/C、重复/同目标/源缺失逻辑；最小单 PATCH 请求测试；真实 move 预览 | 独立测试库实际移动及 children/UI 不变 |
| AC3 | schema 校验、填空/覆盖/拒绝空值测试；真实空 DOI/非空 title 预览 | 真实字段写入 |
| AC4 | 有序结构化个人/机构作者、角色校验、完整数组和回读顺序测试 | 真实作者写入和 UI 表示 |
| AC5 | 预览无授权、拒绝授权、错误 Server ID/版本、组库路径和 HTTP 错误 mock | 真实只读 group、授权 UI、实例重启 |
| AC6 | accepted+回读、mismatch、5xx、实际请求超时、401 一次重授权测试 | 真实应用提交期间断连 |
| AC7 | 真实 SQLite busy 同时 HTTP 预览成功 | 已有证据满足解耦要求 |
| AC8 | Web/connector 回归、doctor 不授权、密钥隔离/脱敏、文档及镜像检查 | 真实记住授权并重启复用 |
| AC9 | 自动化门禁通过 | 独立测试库真实授权/修改/UI 验收 |

可执行后续验收：在独立测试库准备 A/B/C 分类和一个有子笔记/附件的普通
条目；先 deny 验证无变化，再人工 Allow 测试单次字段/作者和 A→B 移动；
每次从本地预览取 server/version，核对 API 回读与 UI，最后记录实例重启、
记住授权、并发编辑和只读 group。不得用主库文献或猜测的元数据作写入 fixture。
Linux/macOS、Rust 1.85 本地运行和 Zotero Undo/BBT 重算尚未验证。
