# Implementation Plan — 实施与验证记录

当前 task.status = in_progress。2026-10-13 用户明确“批准实施”，按 Codex inline 模式实施。自动化检查与真实预览通过，独立测试库实际写入和 UI 验收仍待完成。

## 进入实施前

- [x] 核实当前代码与运行时；官方搜索、固定版本源码和只读 API 探测完成。
- [x] 用户选择“分类移动＋字段/作者补充”。
- [x] 汇总 PRD、design、research，执行 PRD 收敛。
- [x] 用户在看到最终方案后明确批准实施。
- [x] 重新确认工作区/分支，加载 trellis-before-dev 及相关 spec。
- [x] 批准后才运行 task.py start，按 Codex inline 模式实施/检查。

## 顺序与验收

### 1. Local API 基础层（AC1、AC5、AC7）

- [x] zot-desktop 新 client：probe、schema/目标读取、authorize、单 item 条件 PATCH/回读。
- [x] HTTP 限 loopback、禁跨主机重定向/代理，不共用 Web key；Server ID/local version 显式建模。
- [x] 一次/记住授权；profile + Server ID 保存本地 key；复用秘密封装、配置目录和错误模式。
- [x] mock 覆盖旧版/缺版本、403 disabled、401、拒绝授权、429、412/428、未知结果与脱敏。

### 2. 操作和预览（AC2、AC3、AC4、AC5）

- [x] 覆盖命令增加 backend；local 默认 preview，confirm 带 expected server/version。
- [x] collection add/remove 接 local；新增 local move-item，保留其他归属并用单 PATCH。
- [x] item update 支持 schema 校验、fill-empty/显式 overwrite、结构化 creators 文件。
- [x] 纯逻辑测试覆盖 A/C→B/C、不复制 item、同源目标、目标已含/源缺失、非法字段、作者顺序和空值。
- [x] AppContext local 路径不调用 local_library()/remote()；Web 默认行为回归。

### 3. 诊断、错误与自然语言流程（AC6、AC8）

- [x] doctor 区分可达/支持/凭据/未验证，不弹授权框或试写。
- [x] JSON 沿用 envelope，区分 preview/unchanged/applied/conflict/failed/unknown 与回读失败。
- [x] 更新 canonical skill、evals/提示样例、双语 docs、相关 spec/AGENTS 边界。
- [x] 本地 key 不进 config show、Debug、日志或 fixture；区分 native citationKey 和 item key。

### 4. 验证与评审（AC9）

- [x] `cargo test -p zot-desktop --locked`
- [x] `cargo test -p zot-core --locked`（涉及配置/错误时）
- [x] `cargo test -p zot-cli --locked`
- [x] 有意修改 canonical skill 后 `just skills-sync`，再 `just skills-check`；不无故 version-sync。
- [x] `just ci`；修改 docs 后 `npm --prefix docs run build`。
- [ ] 获得测试库写授权后验证 Zotero 10.0.6：只读预览→人工 Allow/Deny→条件写→UI/API 回读；字段、作者、分类分别验证。
- [ ] 测试无 Web key、SQLite snapshot busy、预览后人工编辑、实例变化、只读 group 和断连；不用真实主库条目作破坏性 fixture。
- [x] 记录未覆盖平台/版本及 BBT/UI Undo 限制，按 trellis-check 最终审查。

## 风险文件与回退

- context.rs、cli/args.rs：不让旧命令默认换后端或要求新参数。
- zot-core/config.rs、config show/profiles：独立 local credentials，不改旧 Web key，不恢复旧 bridge 配置。
- zot-desktop/connector.rs：Connector 保持 import-only，新增能力独立成模块。
- zot-remote/zotero.rs：只复用必要纯逻辑，不扩展为全 transport 重构。
- 代码回退可停用显式 local 路径；已写 Zotero 数据不会随代码回退撤销，需独立校验后的反向修改。

## 本轮检查范围

见 [validation.md](validation.md)。真实桌面只执行 GET/preview/unchanged，没有请求本地授权或发送 PATCH。尚未安装新版本到全局 zot，开发二进制位于 target/debug/zot.exe。
