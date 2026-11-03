# 隔离桌面验收方案

核心流程已按授权完成：标签、分类创建、批量整理、UI 和正常重启回读通过，原实例已恢复。用户要求聚焦核心，未执行第 6 项 UI 制造版本漂移；该保护由自动化证据覆盖。拒绝授权已真实通过；以 validation.md 为最终证据。

## 操作范围

- 正常关闭当前 Zotero并记录实例身份；验收结束恢复。不强制结束有未保存编辑的窗口。
- 新建 `D:/Documents/Code/CLI/zotero-cli/target/local-organization-acceptance/2026-11-02/profile` 与独立兄弟 `data` 目录，明确配置实际 dataDir；不能仅新建 profile 就声称隔离。
- 启动独立 profile，生产 CLI 仍连接 23119。核对不同 Server ID、实际 dataDir、空库；不复制主库或凭据，不登录同步。
- CLI 保持 `cargo run -q -p zot-cli -- --json`；ZOT_DATA_DIR 仅对子进程设置。凭据缓存按实例 ID 隔离，首次必须无缓存；用户选择记住授权时仅由现有正规流程保存，不复制或打印密钥。
- 用户本人启用本地通信，处理 Deny / Allow / Always Allow；Agent 不点击授权按钮。

## 数据与顺序

1. 用 Zotero 自身 UI/开发工具建立 3 篇明确标记为测试的 journalArticle、两个控制分类、一个子笔记和一个 example.invalid 链接附件。记录实际 key/Server ID；不下载文献，不直接写 SQLite。
2. A：中文、空格、特殊字符标签新增/移除；保留手工/自动标签。重复添加、移除不存在标签应 unchanged 且无授权；API/UI 核对。
3. B：CLI 预览/创建根分类和子分类，保存真实库版本、返回 key、API/UI 父级；同级重名拒绝，不同父级同名允许。新 key 用于本地成员操作。
4. C：第一篇加“待读”并加入新根分类；第二篇移除旧测试标签并移入新子分类；第三篇加“已读”且保留控制分类。保存实际请求、完整计划/hash，逐项审阅后 apply。
5. 核对生产 CLI 单 envelope、退出 0、落盘报告、API/UI。比较非目标字段、标签类型、其他成员、children 和附件引用快照。
6. 在合成条目 UI 修改后确认旧计划零写拒绝；用户完成拒绝授权场景。仅在独立测试库操作。
7. 正常重启同一隔离 profile，核对分类层级、标签、成员、children 与实际授权复用情况。
8. 正常关闭测试实例，恢复原 Zotero；只读确认原实例身份，不把测试 key 或计划交给主库执行。

## 证据与边界

原始合成 fixture、envelope、快照与 UI 证据留在上述 ignored target 目录，任务内只保存脱敏结果和 AC 映射。自动化故障不在真实库重演；真实组库、其他 OS、实际 Zotero 提交中断继续标为未覆盖。

通过后才能勾选依赖该证据的 AC。此授权不包括 commit、push、PR、真实主库整理或自动点击授权框。
