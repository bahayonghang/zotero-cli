# 最新结论（覆盖下方历史未授权状态）

2026-11-02 核心实现与验收完成，进入 review 待提交收口。Windows Zotero 10.0.7 独立 profile/data：标签增删、根/子分类创建、3 条不同决策批量 apply、API/UI/正常重启回读通过；原实例已恢复。173 个 CLI 测试及 workspace/质量门通过。故障注入与真实桌面证据分列；用户要求聚焦核心，不扩展桌面故障测试。未 commit/push/PR，未归档。见父 validation.md。

真实证据：父任务 acceptance/run.py 与 ignored target/local-organization-acceptance/2026-11-02 下的场景 JSON、plan.json、report.json、restart.json；父 validation.md 汇总 AC 与边界。

# Validation — 本地批量整理

2026-11-02，Codex 接续 Grok 的未提交实现。基线 dev@eb0345f，保持未提交、未推送、未创建 PR。
调用路径固定 `cargo run -q -p zot-cli -- --json`。用户随后已授权隔离桌面验收；测试实例与合成夹具已建立，产品 CLI 尚未请求授权或写入。当前桌面状态以父 validation.md 为准。

## 当前环境（重新探测）

doctor 成功：Windows Zotero 10.0.7、API 3/schema 44 可达且 supported=true；credential_present=false、verified=false；Web 凭据仍缺失。SQLite 快照报 zotero-db-busy。
这取代交接时 Local API 不可达、SQLite 可读的环境快照，不构成桌面验收证据。

## 本轮发现与修复

1. apply 原先信任计划计数及 changed，缺少原始请求、唯一 key、完整 before/after 重算。新增校验；回归测试先失败后通过。
2. no-op 跳过逐项版本检查，分类仅预检一次。现在 no-op 也复查版本，分类逐项及授权等待后复核。分类读与条目条件 PATCH 仍不具备跨对象原子性。
3. 中间磁盘报告计数未刷新，部分落盘错误被忽略，提交后保存失败会抹掉内存已知结果。现在刷新全部计数，所有保存错误停止并非零退出，stdout 保留已知结果，旧磁盘 in-flight 留待人工复核。
4. 分类成员比较可被重复值替换绕过；改为多重集合比较。
5. B 的 collection POST 忽略矛盾/畸形 unchanged 桶；新增失败回归并修为 unknown、不重发。

## 质量门

`just ci` 已通过；最终文档构建、diff 检查及测试数量统一记录在父任务 validation.md。测试均使用临时目录与随机 loopback 端口，不读取真实凭据、不连接桌面实例。

## AC 映射

| AC | 自动化证据 | 生产 CLI / 真实桌面 | 结论 |
| --- | --- | --- | --- |
| B-AC1 三条不同决策、预览零写入、确认匹配 | three_distinct_decisions_preview_then_apply_one_minimal_patch_each：标签添加/移除、成员添加/移除、不同标签，3 次最小 PATCH，全 applied | 成功路径待隔离桌面 | 自动化通过，待桌面验收 |
| B-AC2 计划、实例、库、版本绑定 | edited_plan_cannot_bypass_counts_requests_or_changed_flags；preflight_rejects_drift_in_any_target_before_authorization；分类漂移；精确字节 hash | hash/schema 由真实 CLI 进程验证 | 自动化通过 |
| B-AC3 51 条与非法输入 | fifty_one_changes_require_explicit_matching_limit_and_never_write_in_plan：50 拒绝零写，51 同上限审阅后执行 51 次；伪造计数、重复 key、矛盾动作拒绝 | 请求内混库字段由真实 CLI 拒绝 | 自动化通过 |
| B-AC4 首错停止 | 第二条 500 unknown：第一条 applied、第三条 not-attempted、partial；新增拒绝/冲突停止 | 故障为 mock | 自动化通过 |
| B-AC5 一条一次 PATCH、无关数据不变 | 断言 body 仅计划 tags/collections + version；保留 type=1/未知标签属性和其他成员；无 children/附件请求 | UI、children/附件快照待真实验收 | 自动化通过，待桌面验收 |
| B-AC6 拒绝、401、超时、回读、落盘、中断 | 见下方分层证据 | 无真实 Zotero 提交中断、组库拒绝或授权框证据 | 自动化通过，真实范围未覆盖 |
| B-AC7 兼容、JSON、退出码 | Web batch 回归；报告经 CommandOutput 输出单 envelope + exit 1；main 保留非零码 | 真实 CLI 无 confirm/hash/schema/混库错误通过；完整 partial 报告尚未穿过生产 CLI main | 部分完成 |
| B-AC8 A→B→C 独立库闭环 | 自动化不替代 | 已授权，等待用户开启 Local API | 未完成 |

## 故障证据分层

- HTTP mock：授权 403、PATCH 403/412、401 后一次重新授权、再次 401 停止、accepted 后 GET 500。
- HTTP 超时：扣留 PATCH 回执；回读即使匹配也为 unknown、不重发、停止后续项。不是真实 Zotero 超时。
- 真实文件系统：提交前缺失父目录 => 零 HTTP 请求、全部 not-attempted；提交后把临时报告目录移到保留路径并以普通文件阻断原目录 => stdout 保留 applied/verified，磁盘仍 in-flight，余项未执行，退出非 0。仅操作临时夹具。
- 真实测试子进程：第一项完成，第二项 PATCH 无回执时 kill。磁盘为 applied/in-flight/not-attempted，各计数 1，无重放。子进程是 Rust 测试 harness 运行产品执行器，不是生产 CLI，也不是真实桌面。
- json_error_contract::organize_hash_and_schema_fail_in_real_process_before_network：生产 CLI hash 不符、空目标计划、请求未知 library 字段均单 envelope、exit 1、stderr 空、没有创建报告。
- 合成请求/计划/报告夹具由 organize_tests.rs 生成并校验，临时文件自动清理；没有真实库元数据或凭据入库。

## 剩余工作

独立 profile + data 桌面操作已授权；按父任务 acceptance-plan.md 继续生产 CLI、UI、重启回读。真实组库与其他 OS 未覆盖，不能将 mock 故障记成桌面证据。三个子任务及父任务均不归档。
