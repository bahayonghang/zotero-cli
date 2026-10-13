# Validation — 2026-10-13

实现已提交为 `16ce140`。用户随后授权独立测试库，并亲自完成 Deny、Allow 和
Always Allow；AC9 要求的真实授权/写入/UI 验收已完成。主库已恢复且未写入。
本轮未修改 Rust 产品代码，未推送、未全局安装，用户已确认提交本轮验收材料并归档。

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

实现阶段最后完整检查在超时和作者回读顺序测试补齐后运行。本次真实验收后再次
执行 `just ci`，仍全通过：303 个 Rust 测试和 23 个 Python 测试；未改产品代码。
新增 skill evals 是评估场景，不声称运行过模型评估器。

## 首轮真实 Zotero 10.0.6：只读验证（历史记录）

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

## 独立测试库真实验收

用户在本聊天明确同意使用独立测试库，启用本地通信并手动操作授权框。
运行目录为 `target/local-api-acceptance/01a134db/`，profile 与 data 分开，
没有复制主库数据、登录同步或读取 Zotero 的授权文件。测试实例 Server ID 为
`onZz1IXmgT44`，主库为 `2ZxitCcOtvOu`。全程使用 `target/debug/zot.exe --json`。

虚构 conferencePaper `EVNU399E` 最初属于 A=`YHPN6EZ8` 和 C=`U7ZZKTKE`；
B=`ZD3P2983`。子笔记 `DT273TJG`、链接附件 `5QPFCJF8` 的完整 API JSON 在
所有操作前后相同。附件为 example.invalid 链接元数据，未下载真实 PDF。

| 场景 | 结果 | 条目本地版本 |
| --- | --- | --- |
| 字段/移动预览、已有标题保留、结构字段拒绝 | PASS，无授权、无修改 | 4→4 |
| 用户 Deny | PASS，`local-api-forbidden`，完整快照不变 | 4→4 |
| 用户 Allow，一次性字段补充 | PASS，DOI/摘要/会议名回读一致 | 4→7 |
| 用户 Always Allow，结构化作者 | PASS，个人/机构作者及 editor 顺序、角色一致 | 7→8 |
| A→B 移动，重复移动 | PASS，成员恰为 B/C，重复为 unchanged，其他数据不变 | 8→9 |
| 添加 A、移除 A，各重复一次 | PASS，最小成员变化，重复为 unchanged | 9→11 |
| 已有摘要默认保留，显式覆盖 | PASS，覆盖后的值回读一致 | 11→12 |
| 错误 Server ID | PASS，`local-api-conflict`，快照不变 | 12→12 |
| 保存预览后在 UI 编辑标题 | UI 保存产生版本 13；旧预览被拒绝，publisher 未写入 | 13→13 |
| 同一测试 profile 重启后补日期 | PASS，无新授权框，复用缓存，date=2026 | 13→14 |
| 切回主库后使用测试 Server ID 确认 | PASS，在读取目标/授权/PATCH 前拒绝；主库凭据仍不存在 | 不写主库 |

以上实际成功写入均返回 `state=applied`、`write_accepted=true`、`verified=true`。
UI 在写后和重启后均核对：个人和机构作者、editor、日期、B/C 分类、1 个附件与
1 条笔记正确显示。UI 编辑由 Computer Use 进行，授权按钮由用户亲自点击。
doctor 在真实写通过后仍正确报告 `verified=false`，仅凭据存在变为 true；
Web configured=false，SQLite 仍为 `zotero-db-busy`，不妨碍 HTTP 写入。

证据：`acceptance/results.json` 保存逐场景退出码、状态、版本、子对象不变断言及
原始本地记录 SHA-256。完整 envelope/API 快照和 UI tree 保留于忽略的运行目录。
操作说明见 `acceptance/README.md`；这些是交互式验收材料，不加入无人工授权的 CI。

### 本轮环境问题及恢复

- 沙箱内启动 Zotero 未显示可用窗口；经批准使用桌面进程后正常启动隔离 profile。
- fixture 已创建，但初次导出 manifest 使用 Windows 不接受的路径格式而失败；
  通过同实例只读 GET 恢复 keys，保存的 fixture 改用 `PathUtils` 组装文件路径。
- 初次 Always Allow 后，沙箱无法在 `%AppData%\zot` 保存凭据，CLI 返回 `io`。
  该失败发生在 PATCH 前，作者仍为空；中止后续请求，用户拒绝残留授权框，重新
  预览并以获准的文件访问运行，用户再次授权后完成写入。没有绕过持久化或暴露 key。
- 上一轮用户按 Esc 中止桌面操作后，立即停止；本轮用户要求继续后才恢复操作。
- 结束时正常关闭测试实例并恢复默认 Zotero 主库。隔离 profile、数据和其已授权
  的缓存保留用于复核；测试授权未迁移到主库，未读取或打印 key。

## 验收映射与验证限制

| AC | 当前证据 | 证据边界 |
| --- | --- | --- |
| AC1 | mock 版本/协议 gate；真实无 Web key 授权写入 | 真实运行版本仅 Windows 10.0.6；组库路径有 mock |
| AC2 | 成员边界单测与最小 PATCH；真实 A/C→B/C、add/remove/repeat、children 和 UI 核对 | 附件为链接元数据 |
| AC3 | schema/空值/非法字段单测；真实填空、保留及显式覆盖 | 虚构 conferencePaper fixture |
| AC4 | creators 格式/顺序单测；真实个人、机构、editor 写入及 UI 核对 | 已有作者替换边界由单测覆盖 |
| AC5 | 真实 Deny、旧 UI 编辑版本拒绝、错误实例及切回主库拒绝 | 旧版本、跨库和只读 group 由 mock 覆盖 |
| AC6 | 真实 accepted+匹配回读；mismatch、5xx、超时、401 重授权测试 | 未中断真实 Zotero 提交；故障语义为 mock 证据 |
| AC7 | 真实 SQLite busy 同时 HTTP 写入成功 | 已满足解耦要求 |
| AC8 | Web/connector 回归、脱敏/镜像/文档检查；真实记住授权及重启复用 | doctor 仍不声称验证当前授权 |
| AC9 | `just ci` 通过；独立测试库真实授权/修改/UI 验收完成 | 不以 mock 代替本节真实桌面证据 |

真实同步 group 的可写/只读权限、真实提交期间断连、Linux/macOS、Rust 1.85
本地运行、Zotero Undo 和 BBT 重算尚未验证。相关故障协议有自动化覆盖，以上
不作为已完成的真实环境验证或功能承诺。本次首版验收条件已满足。
