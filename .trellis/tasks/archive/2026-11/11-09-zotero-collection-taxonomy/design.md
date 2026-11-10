# 分类整理方案

## 判断

四个根分类合理。`01Forecasting` 不合理：44 个子类把任务、模型、数据技巧和问题属性放在同一层。侧边栏因此变长，搜索一个主题时文献会散在多个单归属文件夹里。

保留门槛：直接条目不少于 15，并且名字是你会打开的书架。不足 15 的类解散。解散前给每条文献加一个受控标签。

数字是各来源的直接成员数。43 条 Forecasting 文献属于两个子类，净人数在预览时按条目去重，不在这里相加。

## 目标树

`01TimeSeries`（`LJFSL4UY`）收成这些主书架：

```
01TimeSeries
  Forecasting                  Z2YE4E6Y，总架
    Foundation Models          由 LLM/FoundationModels 改名，132
    Transformers               由 Transformer_based_ts_prediction 改名，82
    Multivariate               由 MultiVariable|Variate 改名，46
    Data Augmentation          I2YUP9NE，43
      Diffusion                939UKAVT，22，保留为唯一孙类
    Graphs                     由 Graph Neural Networks 改名，37
    Sequences                  由 Mamba 改名，34，并入 RWKV 1
    Frequency & Decomposition  WEUYPTDU，26，并入 Wavelet 12
  Soft Sensor                  从上移出，ZME8XQMS，118
  Classification               58AYB74E，17。拼写以后再改
  Anomaly & Diagnosis          由 05AnormalyDetection 扩展
  Imputation                   由 06Imputation 扩展
  Analysis                     6QZ3XITA，吸收趋势、生成、性能评估
```

Forecasting 子类从 44 个收到 7 个。TimeSeries 这一层不再把 Soft Sensor、监测和缺失值塞进 Forecasting。

本地 API 不能改父级，也不能改名。执行时创建新名字的分类，把条目移进去，旧类变空后由用户在 Zotero 里删除。能原样保留的类不要新建。

## 来源对照

### 留在 Forecasting 下面

| 来源 | key | 直接数 | 动作 |
| --- | --- | --- | --- |
| LLM/FoundationModels | P5WKAC5Y | 132 | 保留成员。新显示名 Foundation Models |
| Transformer_based_ts_prediction | ZE3Z2YJK | 82 | 保留成员。新显示名 Transformers |
| patch / embed / Attention | GCFFVCIF / 3HMSXAP3 / JMA4AB8Z | 4 / 9 / 1 | 成员留在 Transformers。标签 `ts/patch`、`ts/embed`、`ts/attention`。删空类 |
| MultiVariable\|Variate | Z9BJ9QB5 | 46 | 保留。新显示名 Multivariate |
| Data Augmentation | I2YUP9NE | 43 | 保留 |
| 扩散模型\|Diffusion | 939UKAVT | 22 | 保留为 Data Augmentation 的子类 |
| Graph Neural Networks | HVRA5J27 | 37 | 保留。新显示名 Graphs |
| Mamba | J4PNTSY2 | 34 | 保留。新显示名 Sequences |
| RWKV | PQQT37S6 | 1 | 移入 Sequences。标签 `ts/rwkv` |
| Frequency&Decomposition | WEUYPTDU | 26 | 保留 |
| Wavelet | 3R2KZKPK | 12 | 移入 Frequency。标签 `ts/wavelet` |

### 移出 Forecasting，因为任务不同

| 来源 | key | 直接数 | 落点 |
| --- | --- | --- | --- |
| Soft Sensor 及其 4 个子类 | ZME8XQMS、ND3WMEQQ、ZP6CY7YT、46GBTFQA、58QAX269 | 118 + 11 + 1 + 1 + 10 | 新的 TimeSeries 级 Soft Sensor。子类解散并加 `ts/small-sample`、`ts/machine-learning`、`ts/graph`、`ts/semi-supervised` |
| ProcessMonitoring | 5WELPTAY | 18 | Anomaly & Diagnosis |
| Imputation | HDQ2EUZH | 5 | Imputation |
| MissingValues | 2NXF6MY7 | 9 | Imputation。标签 `ts/missing` |

### 并入 Forecasting 总架

| 来源 | key | 直接数 | 标签 |
| --- | --- | --- | --- |
| 父类现有条目 | Z2YE4E6Y | 14 | 不动 |
| TimeSeriesForecasting | MSKRR445 | 108 | 不加标签。名字与父类重复 |
| Linear / bench | X6T3LS5A / Z6XRVPEP | 2 / 2 | `ts/linear`、`ts/bench` |
| Non-stationaryTS | AD9X6CJI | 14 | `ts/nonstationary` |
| Multi-Modal\|Source | MVTJN75R | 14 | `ts/multimodal` |
| Multi-Scale\|Horizon\|Resolution | S7LAUZAB | 19 | `ts/multiscale` |
| Probabilistic | KAK5GZRM | 11 | `ts/probabilistic` |
| Representation Learning | H25B6H67 | 10 | `ts/representation` |
| Interpretability | 93X55XEE | 10 | `ts/interpretability` |
| irregular sampling | WUB6KYIW | 9 | `ts/irregular` |
| Spatial-Temporal | ZUUGNTI8 | 9 | `ts/spatial-temporal` |
| SCN | U5JATS34 | 9 | `ts/scn` |
| Online | Q3T8UYZJ | 8 | `ts/online` |
| Fuzzy | NYH2W5V4 | 8 | `ts/fuzzy` |
| DistributionShift\|Concept Drift | C29NNG2W | 8 | `ts/distribution-shift` |
| Data&Feature Selection | WDQDMY6T | 7 | `ts/feature-selection` |
| Loss | JYJVG9JD | 6 | `ts/loss` |
| UnBalance/不平衡 | 9J9DDGEF | 5 | `ts/imbalance` |
| FeatureFusion | 6F5WV8XL | 5 | `ts/feature-fusion` |
| Multi-Step | CPEYV8NK | 4 | `ts/multi-step` |
| multi-rate | ABJD8QP5 | 3 | `ts/multi-rate` |
| Denoising | Q4DCFCTU | 3 | `ts/denoising` |
| survey | UJT7RH7T | 2 | 使用已有 `Survey` |
| TransferLearning | BFDQ8NPW | 2 | `ts/transfer` |
| Spiking Neural Networks | M92Y37KP | 2 | `ts/snn` |
| Self-Supervised Learning | UW5XBEKV | 2 | `ts/self-supervised` |
| Robust | JAP6B9JK | 2 | `ts/robust` |
| Distillation | P4F3G6GD | 2 | `ts/distill` |
| DataProcessing | 9HRF9A4A | 2 | `ts/data-processing` |
| ContinualLearning | HIW8XDF7 | 2 | `ts/continual` |
| 机理 | YYHXU3K2 | 1 | `ts/mechanism` |
| SequenceModeling | SZMKKVPR | 1 | `ts/sequence-modeling` |
| KnowledgeDrive | TUSBLASS | 1 | `ts/knowledge-driven` |

### TimeSeries 的其余任务

| 来源 | key | 直接数 | 动作 |
| --- | --- | --- | --- |
| 04Classfication | 58AYB74E | 17 | 保留。拼写留到能改名时 |
| 05AnormalyDetection | HU7D6CU9 | 15 | 作为 Anomaly & Diagnosis 的落点 |
| FaultDiagnosis（其下） | 5TM93XGJ | 10 | 并入该落点。标签 `ts/fault` |
| 08FaultDiagnosis | W8UJ2SNG | 3 | 并入该落点。标签 `ts/fault` |
| 06Imputation | AQVFMLX8 | 4 | 作为 Imputation 的落点 |
| 06DataCleaning | LFYIFTVH | 1 | 并入 Imputation。标签 `ts/data-cleaning` |
| 02Analysis | 6QZ3XITA | 11 | 保留为 Analysis |
| 03Generation | 759ZGSUM | 2 | 并入 Analysis。标签 `ts/generation` |
| 09TrendExtraction | B85MT3V6 | 1 | 并入 Analysis。标签 `ts/trend` |
| 07PerformanceAssessment | 5DPM5LFR | 1 | 并入 Analysis。标签 `ts/performance` |

## 第一批的另外两个领域

页面面板 G 把 Deep Learning 与 Optimization 纳入第一批。规则与 Forecasting 相同。这两个父类内部几乎没有多归属，直接数可以相加。Optimization 只有 1 条多归属，预览时去重。

Control 子树 19 条，不进入第一批。Industry 里的水泥工艺类是项目书架，不按 15 篇门槛解散。

### Deep Learning

父类 `03Deeplearning` = `2M2HMWYL`，直接 0，子类 26，子树 173，内部重叠 0。收到 6 个子类：

| 保留 | key | 直接 | 动作 |
| --- | --- | --- | --- |
| 00Basic | 5FFJS7PN | 8 | 小类落点。已有子类 Representation Learning（`IQ7VHTJ8`，1）解散，标签 `dl/representation` |
| 01Attention | 3P6VQBP9 | 27 | 保留 |
| 02Transformer | S94BVLDT | 23 | 保留。子类 Wavelet（`IT4IVRVT`，4）与同级 `21 Wavelet`（`57T3KNQL`，1）并入这里，标签 `dl/wavelet` |
| 03KAN | ZINQTCE4 | 18 | 保留 |
| 07Mamba | NQTPMVH9 | 10 | 保留在深度学习。不并入 Forecasting 的 Sequences |
| 13Diffusion | IE5HSEMT | 15 | 保留。不并入 Forecasting 的 Diffusion |

并入 `00Basic` 的来源：

| 来源 | key | 直接 | 标签 |
| --- | --- | --- | --- |
| 14MetaLearning | 7ZWBQZUZ | 3 | `dl/meta-learning` |
| 16PINN | 5PRLUYLC | 5 | `dl/pinn` |
| 04RNN | LV94LZEV | 6 | `dl/rnn` |
| 19解释/Explain | YJT493BD | 1 | `dl/explain` |
| 06BigModels | QV4W4T7P | 4 | `dl/big-models` |
| MOE | 52IMKB43 | 10 | `dl/moe` |
| 18超参数 | ZERW3BLH | 1 | `dl/hyperparameter` |
| 09Joint-Training | ZN7BT294 | 5 | `dl/joint-training` |
| 15TabularData | V5RWIH2T | 1 | `dl/tabular` |
| 23RL / MO | D5MI5QAE / SV974DWY | 0 / 2 | 子类 2 条用 `dl/rl`。空的父类稍后在界面删除 |
| 20RL | WZ6Y2UNU | 3 | `dl/rl` |
| 00LLM | D9QAYF5E | 6 | `dl/llm`。不并入 Forecasting 的 Foundation Models |
| 17知识图谱 | BQL9N5NK | 1 | `dl/knowledge-graph` |
| 10Echo State Networks | W2Q69EM5 | 1 | `dl/esn` |
| 08Multi-Task Learning | NSPIFD9R | 10 | `dl/multi-task` |
| 05CNN | BM9UHJUB | 3 | `dl/cnn` |
| 12Visual | EHNGN746 | 1 | `dl/visual` |
| 22FiLM | I7EXK2G4 | 1 | `dl/film` |
| 11Spiking Neuron Network | WAFWBS7A | 1 | `dl/snn` |
| 24WorldModel | 4YU8MM4C | 1 | `dl/world-model` |
| DeepResearch | 2ND8GB6R | 0 | 空类，界面删除 |

`00Basic` 现有子树 9 条。上表除 Wavelet 以外的直接数合计 66，加上这 9 条，落点约为 75 条。Wavelet 的 5 条去 Transformer。重叠为 0，所以这里不用再去重。

### Optimization

父类 `02Optimization` = `C929JUTG`，直接 3，子类 18，子树 160。收到 6 个子类：

| 保留 | key | 直接 | 动作 |
| --- | --- | --- | --- |
| 09DecisionMaking | 9LHF832J | 22 | 保留 |
| 02IndustryOperationOptimization | S5TLAUJ4 | 27 | 保留 |
| 10优化控制/OptimalControl | FLBJ5GXN | 40 | 保留。不并入 Control 的 OptimalControl |
| 06RL | HETF8M4H | 18 | 保留。空子类 MO（`X6RWBDRM`）在界面删除 |
| 12RTO | JDQEPIPG | 14 | 保留。低于 15，但是已定的例外 |
| 01Multi-objective Optimization | 2S5SNU3A | 8 | 保留。子类 DynamicMOP（`L83BTEH8`，5）并入本类，标签 `opt/dynamic` |

并入 Optimization 父类的来源：

| 来源 | key | 直接 | 标签 |
| --- | --- | --- | --- |
| 08Multi-Modal Optimization | WP5G9J7F | 1 | `opt/multimodal` |
| 11调度 | JDSA7A9V | 5 | `opt/schedule` |
| 08Schedule | Z58T79HM | 1 | `opt/schedule`。与调度合计 6，不单设文件夹 |
| 03RobustOptimization | QU6DV7IH | 1 | `opt/robust` |
| 16SeqOptimization | G8WTN4S9 | 1 | `opt/sequential` |
| 13process parameter optimization | U2HWXZEP | 3 | `opt/process-parameter` |
| 15Dynamic | CCCMGVJA | 1 | `opt/dynamic` |
| 17Evolutionary Generative Optimization | YDUUPUIJ | 1 | `opt/evolutionary-generative` |
| 14LLM | LLF6KEBI | 2 | `opt/llm`。不并入 Forecasting |
| 04NN | EAGU9IQT | 1 | `opt/nn` |
| 07OptimizationAlgorithms | EPGXDNU2 | 6 | `opt/algorithms` |
| 05GANDecision | A3974ZQE | 0 | 空类，界面删除 |

这 11 个非空来源的直接数合计 23。父类原有 3 条。多归属只有 1 条，净人数以预览为准。

## 不合并的同名类

这些名字相近，主任务不同，保持两处：

- Forecasting/Mamba 与 Deep Learning/07Mamba
- Forecasting/Wavelet 与 Deep Learning 下的两处 Wavelet
- Forecasting/LLM 与 Optimization/14LLM、Deep Learning/00LLM
- TimeSeries 故障诊断与 Industry/04FaultDiagnosis
- Optimization/10优化控制 与 Control/03OptimalControl
- Soft Sensor/semi-supervised 与 Archive 里刘金波小论文的 semi-supervised

## 标签

新标签只用 `ts/`、`dl/`、`opt/` 前缀加小写短横线。一个被解散的文件夹只对应一个标签。落点分类的名字已经表达该侧面时，不再加同义标签。`TimeSeriesForecasting` 就是这种情况。

不改这些流程标签：`/unread`、`CodeAvailable`、`Chinese_Interpretation`、`Reference`、`📒`、`🤖️`、`/AI文献解读`、`/划词翻译`。

## 兼容与回退

移动使用 `collection move-item --write-backend local`，标签使用 `item tag add --write-backend local`。两者都先预览。确认参数只来自同一次预览。

本地不能删除旧类。回退是把条目移回原 key，并去掉这次加的 `ts/`、`dl/` 或 `opt/` 标签。流程标签和其他分类不动。Archive、书库、Industry 与 Control 不进入批次。
