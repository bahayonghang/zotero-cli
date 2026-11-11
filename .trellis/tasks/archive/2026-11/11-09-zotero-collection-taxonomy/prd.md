# 整理 Zotero 科研分类

## Goal

给个人 Zotero 库一份可执行的分类整理方案。用户要判断截图里 `01Forecasting` 这一支是否过细，以及怎样收束。本任务交付方案和迁移对照，不改库、不改 `zot` 代码。

## Confirmed facts

证据来自 2026-11-10 的 SQLite 快照，命令是 `zot --json doctor`、`collection list`、`library stats`、`library tags`、`library list --limit 2500`、`library search --title`。`library list` 与标题检索的 `meta.read_source` 为 `snapshot`。当时 Local API 与 connector 都不可达，Web API 凭据未配置。数据目录是本机 Zotero 数据目录，配置文件是本机 `%AppData%\zot\config.toml`，schema 131。快照源文件修改时间 `2026-11-10T03:12:55Z`。

截图里 Forecasting 可见子类的侧边栏数字与快照一致。侧边栏 My Library 显示 1797。快照统计的非附件、非笔记顶层条目是 1793。差 4 条，原因未再拆。

| 事实 | 值 |
| --- | --- |
| 分类数 | 175 |
| 顶层条目 | 1793 |
| 分类成员关系 | 2018 |
| 只属于 1 个分类的条目 | 1614 |
| 属于 2 / 3 / 4 个分类 | 130 / 40 / 6 |
| 未入任何分类 | 3 |
| 最大深度 | 5（深度 3 有 80 个分类，深度 4 有 58 个） |

条目几乎是单归属。分类目前承担主书架，不是侧面标签。

根上的四层架子是合理的：`00_Inbox`、`01_Project`、`02_Area`、`03_Resource`。Inbox 与 Project 都是空的。过细发生在 `02_Area / 01ScientificResearch` 里面。

`01TimeSeries`（`LJFSL4UY`）子树 994 条，其中 `01Forecasting`（`Z2YE4E6Y`）占 929 条、直接条目只有 14、子类 44 个。Forecasting 内部同时放了任务、模型、数据技巧和问题属性。同一 Forecasting 子树里只有 43 条同时属于两个子类。

44 个子类里，直接条目不少于 15 的有 11 个，合计 663 条成员关系。少于 15 的有 33 个，合计 189 条。另有 168 条的 Forecasting 归属只在父类或这些小类里。Deep Learning（173 条，26 个子类）内部重叠为 0。Optimization（160 条，18 个子类）内部重叠为 1。

标题含 `MoE` 的 16 条里，5 条在 `MOE`（`52IMKB43`，位于 Deep Learning），3 条是 MOEA/D 的子串误命中，8 条在其他分类。写成 Mixture of Experts 的条目还落在 Wavelet、GNN、LLM、Multi-Modal。

已有标签是阅读流程，不是主题：`/unread` 509、`CodeAvailable` 313、`📒` 273、`/AI文献解读` 197、`/划词翻译` 183、`🤖️` 158、`Chinese_Interpretation` 155、`Reference` 59。主题标签几乎没被使用。解散文件夹时如果不写标签，关键词会从库里消失。

本地写路径不能重命名、删除或改父级。能做的是本地创建分类、移动条目、加减标签。删空文件夹要在 Zotero 界面做，或以后配置 Web API。

明细与 key 见 [research/library-taxonomy.md](research/library-taxonomy.md)。目标树见 [design.md](design.md)。

## Requirements

| ID | 要求 |
| --- | --- |
| R1 | 方案保持四个根分类，不重做 Inbox / Project / Area / Resource |
| R2 | 分类只表示主书架。一条文献一个主分类 |
| R3 | 新的常设子类必须是会按名字打开的书架。直接条目少于 15 的关键词不再单独建类 |
| R4 | 被解散的文件夹先写成一个受控标签，再移动条目 |
| R5 | 第一批同时收 TimeSeries、Deep Learning、Optimization。Forecasting 从 44 个子类收到 7 个，Soft Sensor 上移。Deep Learning 从 26 个收到 6 个。Optimization 从 18 个收到 6 个 |
| R6 | 同名概念才合并，例如 `TimeSeriesForecasting` 并入 Forecasting 父类，Wavelet 并入 Frequency。不把 Deep Learning 的 Mamba 并进 Forecasting 的 Mamba |
| R7 | Archive、课题组论文、thesis 参考文献和 `03_Resource` 不在第一批 |
| R8 | 不改 `/unread`、`CodeAvailable`、`Chinese_Interpretation`、`Reference`、笔记表情和划词标签 |
| R9 | 真正改库时走已文档化的本地预览和确认。每批默认不超过 50 条。不写 `zotero.sqlite`，不自动改走 Web |

## Acceptance Criteria

方案本身：

- [x] AC1 / R1–R8：`design.md` 给出目标树、保留门槛、来源到落点的对照，以及不合并的跨领域同名类
- [x] AC2：研究笔记记录快照时间、`read_source`、key、计数和 MoE 散落证据
- [x] AC3：本任务没有 Zotero 写入，也没有产品代码改动

以后执行时才检查：

- [ ] AC4 / R4–R6：用户批准某一批后，该批条目的标签和主分类与预览一致，其他分类和流程标签不变
- [ ] AC5 / R9：预览为零写入。Local API 不可达时停止。失败批次不能说成整批成功
- [ ] AC6：执行后重新 `collection list` 与 `library stats`。Forecasting 子类为 7 个，外加 Diffusion。Deep Learning 子类为 6 个。Optimization 子类为 6 个。空的旧类由用户在界面删除

## Key decision

页面面板 G 选择「三个领域一起收」。TimeSeries、Deep Learning、Optimization 使用同一条 15 篇门槛，放进第一批。Archive、Industry、Control 和书库仍不进入。

低于门槛但仍保留的文件夹只有三处：Deep Learning 的 Mamba（10，不并入 Forecasting）、Optimization 的 RTO（14）、Multi-objective（直接 8，加上 DynamicMOP 5）。MOE（10）不在例外里，并入 `00Basic`，标签 `dl/moe`。`11调度` 与 `08Schedule` 合计 6 条，不单设文件夹。

随后批准「按预览改库」。2026-11-10 03:58Z 的 `zot --json doctor` 里 `local_api_write.reachable` 为 false，错误是 `local-api-unreachable`。Web 凭据也没有。按 R9 停在预览之前，库没有写入，也没有运行 `task.py start`。Zotero 打开并允许本地通信之后，从第一个源分类重新 doctor，再预览。

## Out of scope

改 `zot` 代码、自动分类器、语义重排、整库清洗、Archive 与书库、Industry 与 Control、跨库移动、未批准的写入、直接 SQL、删除非空分类、批量改名。
