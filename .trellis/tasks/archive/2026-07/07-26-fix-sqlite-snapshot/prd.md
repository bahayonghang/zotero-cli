# P1: SQLite 安全快照（移除 immutable=1 与手工 copy）

## Goal

让每次 `LocalLibrary::open` 都从 Zotero 正在使用的数据库创建事务一致的只读临时快照，
消除 `immutable=1` 对 live DB 的错误假设和 DB/WAL/SHM 分步复制；锁竞争必须明确失败，
不得把不一致或损坏的文件伪装成成功快照。

## Background

- 审计报告 QW-05/M-01（`zotero-cli-code-audit-2026-07-25.md:60-61,119,735-744,766-773`）
  已确认 `src/zot-local/src/db.rs:1458-1490` 使用
  `mode=ro&immutable=1` 读取 live DB，失败后手工复制 DB/WAL/SHM，且 sidecar copy error
  被忽略。这两条路径都无法证明一致性。
- 父任务 `07-26-audit-remediation/prd.md:26,45-50` 将 QW-05 与 M-01 全部分配给本任务；
  因此仅删除 `immutable=1` 不是完整验收。
- `zotero.sqlite` 仍是 Zotero-owned source；本任务只读源连接并写入 task-owned 临时数据库，
  不得对源库执行 schema、checkpoint 或数据写入。
- rusqlite 0.39 的 `backup` feature 提供 `backup::Backup::new` 与 `step`；`step` 将
  `SQLITE_BUSY`/`SQLITE_LOCKED` 暴露为可重试结果，适合建立有截止时间的显式策略。

## Requirements

### R1 普通只读源连接

- 通过 `SQLITE_OPEN_READ_ONLY` 直接打开源路径，不构造 URI，不使用 `immutable=1`。
- 源连接设置 5 秒 busy timeout；不得复制或单独处理 `-wal`/`-shm` 文件。
- SQLite busy/locked 在打开、backup init/step 或验证阶段统一映射为
  `ZotError::Database` code `zotero-db-busy`，hint 提示关闭 Zotero 或稍后重试。

### R2 SQLite Backup API 快照

- 在 task-owned `TempDir` 内创建目标 `zotero.sqlite`，使用 rusqlite Backup API 从只读源
  分页复制；每步让出时间，并以 5 秒单调时钟 deadline 限制 busy/locked 重试。
- backup 完成前不得把目标交给查询层；完成后关闭可写目标连接，以 READ_ONLY 重新打开。
- 快照必须执行 `PRAGMA quick_check`，非 `ok` 结果返回稳定
  `zotero-db-snapshot-integrity`；失败时 `TempDir` 自动清理。
- `LocalLibrary` 生命周期持有 `TempDir`，保证查询期间快照文件存在。

### R3 快照元数据与可观测输出

- 新增可序列化 `LibrarySnapshotMeta`，至少包含 source DB mtime、snapshot UTC time 和
  Zotero `userdata` schema version；source mtime 不可取得时明确为 `null`，不得伪造。
- `LocalLibrary::snapshot_meta()` 返回本次打开对应的不可变元数据。
- `zot --json doctor` 的 `capabilities.local_sqlite_read.snapshot` 输出该结构；human doctor
  至少打印 snapshot time 和可用的 source mtime/schema version。

### R4 兼容性与范围

- 保持 `LocalLibrary::open(data_dir, scope)`、`db_path()` 和所有查询 API 兼容；`db_path()`
  继续指向源 `zotero.sqlite`，不暴露会被清理的临时路径。
- 只给 workspace `rusqlite` 增加现有 crate 的 `backup` feature，不新增外部依赖。
- 不实现父任务范围外的 L-02 `snapshot/` 模块拆分、长驻快照缓存、live-read feature flag、
  telemetry 或跨命令快照复用。

## Acceptance Criteria

- [x] 源连接代码中不存在 `immutable=1`，不存在 DB/WAL/SHM 手工复制或被忽略的 copy error。
- [x] WAL fixture 证明未 checkpoint 的已提交数据进入快照，且源库未被写入或 checkpoint。
- [x] 并发 writer 压力测试反复创建快照，跨表不变量始终成立，`PRAGMA quick_check='ok'`，
      不出现 `SQLITE_CORRUPT` 或半提交视图。
- [x] 锁竞争在测试 deadline 后返回 `zotero-db-busy` 与可操作 hint，不无限重试或 fallback。
- [x] metadata 测试覆盖 source mtime、snapshot time、schema version；doctor JSON 精确包含
      `capabilities.local_sqlite_read.snapshot`，失败 capability 仍保留 typed error envelope。
- [x] `cargo test -p zot-local`、相关 `zot-cli doctor` 测试和最终 `just ci` 全部通过。

## Out Of Scope

- 修改 Zotero schema、强制 WAL checkpoint、写入源数据库或复制 attachment storage。
- 10,000 次长期 benchmark gate；本任务使用有界、可重复的并发回归测试，长期性能预算另行立项。
- 通用 connection pool、application/use-case 层、L-02 god-object 拆分或 observability 系统。
