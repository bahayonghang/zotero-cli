# 库分类证据

日期：2026-11-10。调用面：已安装的 `zot --json`。没有写入。

## 读取条件

`zot --json doctor`：

- `data_dir`: 本机 Zotero 数据目录
- `config_file`: 本机 `%AppData%\zot\config.toml`
- `schema_version`: 131
- `local_sqlite_read.snapshot.source_modified_at`: `2026-11-10T03:12:55.219736100Z`
- `local_sqlite_read.snapshot.snapshot_created_at`: `2026-11-10T03:14:56.690966700Z`
- `desktop_read`: 不可用，`local-api-unreachable`
- `connector_write`: 不可用，`connector-unreachable`
- `web_write.configured`: false

`library list --limit 2500` 的 meta：`count=1793`，`total=1793`，`trash_policy=excluded`，`read_source=snapshot`。

`library search --title MoE` 的 meta 同样是 `read_source=snapshot`。`collection list` 的 meta 只有 `api_version`，没有 `read_source`。它走同一条本地库路径。

截图中 Forecasting 可见子类计数与下表一致。侧边栏 My Library 为 1797，快照顶层条目为 1793。

## 库级

- 分类 175
- `library stats` 顶层条目 1793
- 分类成员关系 2018
- 归属个数：0→3，1→1614，2→130，3→40，4→6
- 深度直方图：0→4，1→4，2→19，3→80，4→58，5→10

类型：journalArticle 848，preprint 517，thesis 198，conferencePaper 122，book 75，document 23，其余见当次 stats。

## 根

| 分类 | key | 直接 | 子树 |
| --- | --- | --- | --- |
| 03_Resource | 3NGJSKTB | 0 | 79 |
| 02_Area | EUKN35DS | 0 | 1939 |
| 00_Inbox | 7HK9J2NL | 0 | 0 |
| 01_Project | 7FVUG34L | 0 | 0 |

`02_Area` 下：`01ScientificResearch` 子树 1454，`00_Archive` 子树 484，`02AIApplication` 子树 1。

`02课题组论文`（`ULWAG6CW`，53）的父级是 Archive（`AZCB6M2E`），不是 Area 的直接子类。

## Forecasting 子类

父类 `01Forecasting` = `Z2YE4E6Y`，直接 14，子类 44，子树 929。TimeSeries `LJFSL4UY` 子树 994。

不少于 15 的直接子类：

| 名 | key | 直接 |
| --- | --- | --- |
| LLM/FoundationModels | P5WKAC5Y | 132 |
| Soft Sensor | ZME8XQMS | 118 |
| TimeSeriesForecasting | MSKRR445 | 108 |
| Transformer_based_ts_prediction | ZE3Z2YJK | 82 |
| MultiVariable\|Variate | Z9BJ9QB5 | 46 |
| Data Augmentation | I2YUP9NE | 43 |
| Graph Neural Networks | HVRA5J27 | 37 |
| Mamba | J4PNTSY2 | 34 |
| Frequency&Decomposition | WEUYPTDU | 26 |
| Multi-Scale\|Horizon\|Resolution | S7LAUZAB | 19 |
| ProcessMonitoring | 5WELPTAY | 18 |

少于 15 的 33 个直接子类合计 189。名单与 key 在 `design.md`。

孙类额外直接数 63：Diffusion 22，Transformer 的 patch/embed/Attention 14，Soft Sensor 的四个子类 23，TimeSeriesForecasting 的 Linear/bench 4。

Forecasting 子树归属个数：0 个子树分类的库内条目 907，恰好 1 个 843，恰好 2 个 43。没有 3 个。168 条的 Forecasting 归属只落在父类或不足 15 条的子类。

共现最高是 TimeSeriesForecasting + LLM，3 条。其余对最多 2 条。

## 另外两处过细的父类

| 父类 | key | 直接 | 子类 | 子树 | 内部多归属 |
| --- | --- | --- | --- | --- | --- |
| 03Deeplearning | 2M2HMWYL | 0 | 26 | 173 | 0 |
| 02Optimization | C929JUTG | 3 | 18 | 160 | 1 |

空类：`05GANDecision` `A3974ZQE`，`06RL/MO` `X6RWBDRM`，`DeepResearch` `2ND8GB6R`，Inbox，Project。

## 标题 MoE

`library search --title MoE` 共 16 条。

在 `MOE`（`52IMKB43`）的 5 条：`UNY5DITP`、`CZ5G8R3F`、`6MCGWL6X`、`3QJUC8NN`、`3G5GDQB4`。`6MCGWL6X` 还在 Multivariate 与 Soft Sensor。

MOEA/D 误命中 3 条：`LEVGTA8G`、`IU95P7VQ`、`C38XS47V`，位于 Archive 的 references、references2、开题。

其余 8 条的主分类：Classification、Non-stationaryTS、LLM/FoundationModels（2）、扩散模型、07Mamba、Frequency&Decomposition、13Diffusion。

` --title "Mixture of Experts"` 12 条，`--title "Mixture-of-Experts"` 7 条。其中落在 MOE 之外的包括 Wavelet `ZQ6KK25X`、GNN `SWHLKE8N`、LLM 数条、Multi-Modal `3SXCVKSW`、Multivariate `LH3IJ8MC`。

## 流程标签

`library tags`：`/unread` 509，`CodeAvailable` 313，`📒` 273，`/AI文献解读` 197，`/划词翻译` 183，`🤖️` 158，`Chinese_Interpretation` 155，`Reference` 59。同一次快照上的 `library stats` 对 `/unread` 是 508，对 `CodeAvailable` 是 312。两次命令不是同一进程。方案引用 `library tags`。主题标签最高的是 `nn/model architecture` 21 与 `nn/Mamba` 8。
