# 最终验收映射 — 2026-11-02（本机日期）

基线 dev@eb0345f；所有产品改动未提交、未推送、未创建 PR。A/B/C 及父任务进入 review：核心实现、质量门与隔离桌面验收已完成，待提交及归档收口。没有直接写 SQLite 或对用户主库执行 mutation。

## 核心桌面结果

用户明确授权隔离验收，并亲自启用 Local API、处理 Deny / Allow / Always Allow。环境为 Windows、Zotero 10.0.7、API 3；独立 profile/data 位于 ignored target/local-organization-acceptance/2026-11-02。测试 Server ID OUeXA4qohacD，与原实例 2ZxitCcOtvOu 不同。CLI 固定 cargo run -q -p zot-cli -- --json，未提供 Web 凭据。

- fixture：3 条合成 journalArticle、2 个控制分类、1 条子笔记、1 个 example.invalid 链接附件；通过 Zotero 自身创建，未复制主库。
- preview PASS：新增预览、重复自动标签、移除不存在标签、错误实例拒绝，数据无变化。
- deny PASS：真实返回 local-api-forbidden，数据无变化。此前另一次等待超时也保存了无变化快照；该超时不计作拒绝通过。
- tag-add/tag-remove PASS：待读、needs review、C++/安全 新增与移除、重复操作 unchanged；保留 automatic-control type=1、manual-control 和其他标签。
- create PASS：根分类 CPSZ4T2V、子分类 84EV2YIA；同级重名拒绝、不同父级同名允许；新 key 可直接用于 collection add-item。
- organize-plan PASS：3 条、6 个动作、完整 before/after；计划阶段快照不变。
- organize-apply PASS：3 applied、0 failed、0 unknown、0 not-attempted，每项 verified=true；退出 0，单 JSON envelope，磁盘 report.json 与 stdout 报告一致。
- UI：根分类显示 paper 1 和待读；其子分类显示 paper 2、paper 3 和已读，层级正确。
- restart PASS：正常关闭并重启同一隔离 profile，items/collections/children 全部与最终快照相同，UI 层级和两篇子分类文献保留。
- 恢复完成：测试实例正常退出，重新启动原 profile；无代理只读 API 校验 Server ID 2ZxitCcOtvOu。未向原实例发送测试 key 或写请求。

所有场景都比较非目标 metadata、children、附件引用、自动标签类型与控制分类成员不变。批量计划 SHA-256 为 5390945bf648ad77092fa5b5f0a642a24a7ca868ccefb7dedc663dce222ebb05。原始合成证据见上述 ignored runtime 的 fixture.json、各场景 .json / -events.json、plan.json、report.json、expected-final.json、restart.json；可复查的执行步骤见 acceptance/run.py。UI 证据为本会话 native 窗口截图与 accessibility 观察。

## 自动化与修复

- C：重算计划原始请求/字段/计数/changed，验证每项版本及分类身份（包括 no-op、授权前后）；报告计数、提交前后落盘失败及停止行为；成员按多重集合比较。
- B：拒绝矛盾或畸形 collection POST 结果桶，HTTP 200 不等于对象成功。
- Windows 真实预览发现 item dispatcher 栈溢出。future 原为 68656 bytes，而 tag 分支仅 2912；create/add-doi/add-url/add-file/attach 五个分支 Box::pin 后，16 KiB 预算回归与真实 CLI 预览/写入通过。未实际执行这些 Web 导入上传操作。
- 补齐 3 种不同决策、51 条默认拒绝/显式扩大成功、401 恢复/再次失败、403、412、超时/500/回读失败、提交前后真实文件系统落盘失败、强制终止 Rust 测试子进程等。进程终止夹具不等于生产 CLI 或真实 Zotero 中断。
- just ci：fmt、locked workspace check、严格 Clippy、173 个 zot-cli bin 测试及其余 workspace 测试、版本守卫、skill 镜像通过；另核对 just skills-check 的 23 个 Python 测试通过。日志 1793660092_just_ci.log。曾因正在运行的 zot.exe 被 Windows 锁定而链接失败，进程退出后的重跑通过。
- 双语文档构建此前通过，最终验收状态文案更新后 npm --prefix docs run build 再次通过。git diff --check 通过。

## AC 映射与证据边界

| ID | 结论 | 证据 |
| --- | --- | --- |
| R1 / AC1 | 通过 | 真实标签增删/API/UI/重启，其他信息保留 |
| R2 / AC2 | 通过 | 真实根/子分类与返回 key、同名策略；父级/版本漂移自动化 |
| R3 / AC3 | 通过 | 真实生产 CLI 三条不同决策计划/apply/报告/API/UI |
| R4 / AC4 | 通过 | 真实零写预览、错误实例、拒绝授权；错误库、过期版本、只读组库自动化 |
| R5 / AC5 | 通过 | 注入失败/未知/超时/中断的状态与停止行为；不声称真实桌面故障 |
| R6 | 通过 | 51 条上限、报告文件系统失败、测试进程中断证据 |
| R7 / AC6 | 通过 | 质量门、双语文档、canonical skill 与镜像 |
| AC7 | 通过 | 独立 profile/data、真实授权/写入/UI/重启、恢复原实例 |
| AC8 | 通过 | 双语说明已按实际证据更新，支持范围与限制明确 |

用户要求聚焦核心、不扩展桌面故障测试。因此未执行 save-stale/reject-stale 的 UI 制造版本漂移；版本保护由已有自动化覆盖。真实组库、其他 OS、真实 Zotero 提交中断、桌面提交后报告落盘失败仍未覆盖。无跨条目原子事务、自动回滚或盲重试承诺。

## 交付状态

功能和上述验收已完成。工作区保留全部改动，任务为 review；不运行会自动提交的 archive/add_session。后续提交、推送或 PR 需要用户明确指令；未进行这些动作。
