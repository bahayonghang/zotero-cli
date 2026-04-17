# LLM 在时间序列中的应用综述

## Source Metadata

- dataSource: Zotero collection `LLM/FoundationModels`
- dataSource url: 当前命令输出未提供 canonical collection URI；本次综述基于 collection key `P5WKAC5Y` [需确认]
- coverage summary:

| 指标 | 数值 | 说明 |
|---|---:|---|
| count | 114 | collection 内全部条目 |
| fulltext_count | 0 | 当前环境 `pdf_backend.available=false`，本次未形成 `fulltext` 证据 |
| metadata_only_count | 114 | 主体分析基于 `metadata+abstract` |
| annotations_count | 0 | 当前环境下未稳定提取 `annotations` |
| notes_count | 91 | 通过 `zot --json item get <ITEM_KEY>` 统计到带 `existing notes` 的论文数 |

本综述是一份 broad review。由于当前环境无法稳定抽取 `fulltext`，所以多数判断来自 `metadata+abstract`，并以 `existing notes` 作为补充证据。凡是涉及强因果判断、严格 SOTA 归因、或真实工业效果外推的结论，均应视为证据受限；相关表述在必要时标记为 `[需确认]`。

## 背景与当前状态

这个 collection 展现出一条相当清晰的演化主线：2024 年的核心任务是把通用语言模型迁移到时序预测场景，主要手段包括 reprogramming、prompt-based adaptation、cross-modal alignment、retrieval augmentation，以及 patch/token 级表示变换；与此同时，native time-series foundation model 也开始快速成形。到了 2025-2026 年，研究重心进一步扩展到 multimodal reasoning、time-series agent、benchmark 治理、以及工业垂域落地。[Jin, 2024, GNTDNEUI] [Liu, 2024, 5XCFMRIS] [Ansari, 2024, 3WJZAR2B] [Shi, 2024, E3RMDSXY] [Goswami, 2024, ANF55ZAR] [Liu, 2026, PC45HAI6]

从任务结构看，forecasting 仍然是绝对主轴，classification、QA、reasoning、agent workflow 和 industrial orchestration 仍属于快速增长但尚未完全稳定的支线。换句话说，领域已经从“LLM 能不能做时序预测”转向“什么样的时间序列基础模型更通用、更可扩展、更可解释，以及 reasoning 是否真的带来稳定增益”。[Tan, 2024, TGW8IKBF] [Li, 2024, NMGEYFXY] [Li, 2025, 4A7VQ9KI] [Liu, 2025, 2H6GBXU3] [Ye, 2025, GS84W4SF]

## 核心方法演进

| 阶段 | 主要方法 | 代表论文 | 这一阶段解决的问题 | 当前局限 |
|---|---|---|---|---|
| LLM 再编程阶段 | 把数值序列映射为 token/prompt，再借用通用 LLM 的先验 | [Jin, 2024, GNTDNEUI], [Liu, 2024, 5XCFMRIS], [Tang, 2025, C6DH7KKX] | 快速验证 LLM 是否能迁移到 forecasting | 对时序归纳偏置弱，常需要额外对齐模块；泛化机制仍偏经验性 `[需确认]` |
| 原生 TSFM 阶段 | decoder-only、encoder-based、MoE、long-context、serial scaling | [Ansari, 2024, 3WJZAR2B], [Shi, 2024, E3RMDSXY], [Goswami, 2024, ANF55ZAR], [Xiao, 2025, 6WFWZ84E], [Liu, 2026, PC45HAI6] | 建立专门面向 time series 的预训练范式与 scaling 路线 | 数据质量、任务统一性和评测协议仍然制约结论可比性 |
| 多模态扩展阶段 | 文本、图像、metadata、paired text 与时序联合建模 | [Li, 2025, 9EER2W65], [Wang, 2024, F2ISE43Z], [Wu, 2025, GJI6A654], [Wang, 2025, GQ3FPJYV] | 引入外部语义、场景上下文和 domain knowledge | 多模态增益高度依赖对齐质量与数据构造，稳定性仍需更多验证 `[需确认]` |
| Reasoning / Agent 阶段 | slow-thinking、R1-style reasoning、TS QA、tool-augmented agents | [Liu, 2025, 2H6GBXU3], [Zhang, 2025, EDNTT2WX], [Guan, 2026, F6Z5YJ9B], [Zhao, 2025, X8EVUCD7], [Ye, 2025, GS84W4SF] | 让模型从“预测器”变成“时间推理器/分析器” | benchmark 与真实任务之间是否一致、reasoning 是否稳定增益，仍然存在争议 |
| 工业落地阶段 | process industry foundation model、small-large collaboration、domain agent | [Ren, 2025, C4RMX6WL], [Ren, 2025, IQ2AIFVZ], [Wang, 2025, 9WTUALLC], [Wang, 2025, VNFDCIHS], [陈致蓬, 2025, EGUTY2KE] | 把 foundation model 接到流程工业、能源、钢铁等具体场景 | 工业效果多来自高层描述与场景宣称，跨场景可复现性仍需 `fulltext` 级验证 `[需确认]` |

从方法史角度看，2024 年的共识更像是在探索“如何把 LLM 用起来”；而 2025-2026 年开始形成另一种共识：真正有竞争力的系统往往需要时间序列原生结构、任务统一预训练、或者显式的 reasoning/agent pipeline，而不是只靠把数字塞进通用 LLM。[Jin, 2024, GNTDNEUI] [Ansari, 2024, 3WJZAR2B] [Shi, 2024, E3RMDSXY] [Liu, 2026, PC45HAI6] [Guan, 2026, F6Z5YJ9B]

## 关键发现与结果

### 1. Forecasting 仍然是评价中心

collection 中绝大多数论文仍然把 forecasting 作为主任务，说明该领域的“基础模型”叙事仍以预测性能为核心，而不是以开放式时序理解为核心。像 Chronos、Time-MoE、MOMENT、TimeFound、Timer-S1 这样的工作都把统一 forecasting 作为主要展示窗口，这让 forecasting 成为目前最成熟、最可比较的主战场。[Ansari, 2024, 3WJZAR2B] [Shi, 2024, E3RMDSXY] [Goswami, 2024, ANF55ZAR] [Xiao, 2025, 6WFWZ84E] [Liu, 2026, PC45HAI6]

### 2. “直接复用 LLM” 正在让位于 “时间序列原生基础模型”

Time-LLM、CALF、LLM-PS、T-LLM 这一线证明了通用 LLM 经过结构重编程、蒸馏或对齐后，确实可以在时序任务上取得竞争力；但同时，Chronos、MOMENT、Time-MoE、Timer-S1、TimeFound 这一线说明，原生 TSFM 似乎更容易形成可扩展的数据-模型-训练范式闭环。[Jin, 2024, GNTDNEUI] [Liu, 2024, 5XCFMRIS] [Tang, 2025, C6DH7KKX] [Guo, 2026, EMKUFPMT] [Ansari, 2024, 3WJZAR2B] [Goswami, 2024, ANF55ZAR] [Shi, 2024, E3RMDSXY] [Liu, 2026, PC45HAI6] [Xiao, 2025, 6WFWZ84E]

这意味着领域的核心问题已经不是“LLM 是否可用”，而是“什么时候该适配通用 LLM，什么时候应该直接训练时间序列原生模型”。仅从当前 `metadata+abstract` 证据看，native TSFM 在统一预训练、长上下文建模和 scaling narrative 上更完整；而 LLM-based 方法在跨模态解释、上下文融合和 reasoning 接口上更灵活。[Tan, 2024, TGW8IKBF] [Cheng, 2025, 4752UGGC] [Liu, 2025, 2H6GBXU3] [Ye, 2025, GS84W4SF] [需确认]

### 3. 多模态不是点缀，而是把外部知识引入时序建模的关键接口

多篇论文都在试图把文本、视觉或 metadata 作为上下文变量引入时间序列预测或理解。`Language in the Flow of Time` 把 paired texts 看作辅助时序变量；ChatTime、Aurora、Time-VLM、ITFormer 则把多模态输入进一步推向统一 foundation model 或 QA/forecast 框架。这说明“数值序列本身不足以承载全部场景知识”已经成为越来越强的共识。[Li, 2025, 9EER2W65] [Wang, 2024, F2ISE43Z] [Zhong, 2025, J36ULJXY] [Wu, 2025, GJI6A654] [Wang, 2025, GQ3FPJYV]

但从现有摘要层证据看，多模态路线的真实增益仍然高度依赖数据构造与对齐机制：Time-MMD、paired texts、multitask QA dataset 一类工作实际上都在强调“高质量数据组织”与“跨模态桥接”本身就是贡献的一半。因此，多模态路线的上限很高，但它不是零成本增强器。[Liu, 2024, 7U722X5F] [Li, 2025, 9EER2W65] [Wang, 2025, GQ3FPJYV] [需确认]

### 4. Reasoning/Agent 是 2025 之后最明显的新增长点

Time-R1、TimeMaster、TimeOmni-1、TimeSeriesScientist、TS-reasoner、Time-MQA、CaTS-Bench 共同表明，社区已经不满足于“给一个 horizon 然后输出预测值”，而是在试图让模型完成多步时间推理、时序问答、分析链路编排与工具调用。这个变化非常重要，因为它把时间序列任务从单一 supervised prediction 推向了 analytical workflow。[Liu, 2025, 2H6GBXU3] [Zhang, 2025, EDNTT2WX] [Guan, 2026, F6Z5YJ9B] [Zhao, 2025, X8EVUCD7] [Ye, 2025, GS84W4SF] [Kong, 2025, 2U932HE2] [Zhou, 2025, K7RNQ6CX]

不过，当前证据也提示了一个潜在风险：很多 reasoning 工作把 benchmark 构造本身当成核心创新之一，这意味着“模型学会了时间推理”与“模型适应了新 benchmark”之间仍然可能没有被完全分开。也就是说，reasoning 是真正的方向，但是否已经成熟到可以取代 forecasting-centered evaluation，仍应谨慎。[Kong, 2025, 2U932HE2] [Zhou, 2025, K7RNQ6CX] [Liu, 2025, 2H6GBXU3] [需确认]

### 5. Benchmark、数据质量与信息泄漏治理正在变成基础设施议题

FoundTS、TSFM-Bench、CaTS-Bench、关于 data quality、information leakage、scaling laws 的论文说明，领域已经意识到：如果没有统一的 benchmark 协议和足够严格的数据治理，foundation model 的结论很容易被训练集偏差、数据重复或任务定义漂移所污染。这说明该领域正在从“模型创新期”进入“模型+基准共同定义期”。[Li, 2024, NMGEYFXY] [Li, 2025, 4A7VQ9KI] [Zhou, 2025, K7RNQ6CX] [Wen, 2024, BAGP3UMG] [Meyer, 2025, U72EAEL6] [Yao, , XUZQVJG8]

## 跨论文比较矩阵

| 主题 | 代表论文 | 共同策略 | 优势 | 当前短板 |
|---|---|---|---|---|
| 通用 LLM 适配 forecasting | [Jin, 2024, GNTDNEUI], [Liu, 2024, 5XCFMRIS], [Tang, 2025, C6DH7KKX], [Guo, 2026, EMKUFPMT] | reprogramming、alignment、prompt、distillation | 能快速继承 LLM 的语义先验与接口能力 | 时序归纳偏置不足，通常依赖额外适配模块 |
| 原生 TS foundation model | [Ansari, 2024, 3WJZAR2B], [Shi, 2024, E3RMDSXY], [Goswami, 2024, ANF55ZAR], [Liu, 2026, PC45HAI6], [Xiao, 2025, 6WFWZ84E] | 时间序列原生 tokenization、预训练目标、MoE、long-context | scaling 路径更完整，更像真正的平台型模型 | 跨任务 reasoning 与解释能力仍在补齐 |
| 多模态时序建模 | [Li, 2025, 9EER2W65], [Wang, 2024, F2ISE43Z], [Wu, 2025, GJI6A654], [Wang, 2025, GQ3FPJYV] | 文本/图像/metadata 与数值序列联合建模 | 能引入上下文与场景知识，提升可解释性接口 | 对数据构造和跨模态对齐要求高 |
| Reasoning / Agent | [Liu, 2025, 2H6GBXU3], [Guan, 2026, F6Z5YJ9B], [Zhao, 2025, X8EVUCD7], [Ye, 2025, GS84W4SF] | CoT/R1 风格、多步推理、工具调用、分析 agent | 直接贴近真实分析流程 | benchmark-真实任务迁移仍需验证 `[需确认]` |
| Benchmark / 评测治理 | [Li, 2024, NMGEYFXY], [Li, 2025, 4A7VQ9KI], [Zhou, 2025, K7RNQ6CX], [Meyer, 2025, U72EAEL6] | 数据集整理、统一评测、描述能力与泄漏治理 | 提高结论可比性与可信度 | 很多评测仍偏 forecasting，尚未完全覆盖 reasoning |
| 工业垂域落地 | [Ren, 2025, C4RMX6WL], [Ren, 2025, IQ2AIFVZ], [Wang, 2025, 9WTUALLC], [Wang, 2025, VNFDCIHS], [陈致蓬, 2025, EGUTY2KE] | domain knowledge、small-large collaboration、agent orchestration | 与真实场景耦合更强 | 可复现公开证据仍不足，工程复杂度高 |

## 收敛点与冲突点

| 问题 | 收敛点 | 冲突点 |
|---|---|---|
| 通用 LLM 是否足够 | LLM 经过合适适配后可成为强基线 | 是否应继续把通用 LLM 当主线，还是转向原生 TSFM，仍未完全统一 |
| 更大模型是否一定更好 | 大数据、长上下文、MoE、serial scaling 普遍被认为有价值 | 轻量化、结构剪枝、small model collaboration 又显示“更大”并非唯一方向 |
| 多模态是否稳定增益 | 文本/视觉/metadata 能补足时序上下文 | 增益依赖对齐质量与数据构造，迁移性仍待验证 |
| reasoning 是否已成熟 | 领域已普遍承认时间推理值得单独建模 | reasoning 的提升是否来自真正的 temporal reasoning，而非 benchmark 定义，仍有争议 |
| 工业应用是否已进入稳态 | foundation model 已开始进入工业议题中心 | 大多数公开论文仍更像“方向验证”，而不是可普适复用的工业标准方案 `[需确认]` |

一个特别值得注意的冲突是：一批论文在努力证明 “LLM + 适配模块” 足以处理时序问题，另一批论文则在事实上把资源投入到了“重新定义时序基础模型”的方向。前者强调模型通用性与接口灵活性，后者强调时序归纳偏置、统一预训练目标与 scaling。这种分裂很可能会持续一段时间，并最终在不同任务上形成分工，而不是由单一路线胜出。[Jin, 2024, GNTDNEUI] [Liu, 2024, 5XCFMRIS] [Ansari, 2024, 3WJZAR2B] [Shi, 2024, E3RMDSXY] [Liu, 2026, PC45HAI6] [需确认]

## 研究空白与未来方向

| 空白 | 为什么重要 | 可能的下一步 |
|---|---|---|
| forecasting 之外的统一任务协议不足 | 当前 benchmark 仍明显偏 forecasting | 建立覆盖 classification、QA、decision-making、agent workflow 的统一任务套件 |
| reasoning 的真实性与稳定性缺少 `fulltext` 级因果证据 | 许多结论仍停留在 benchmark 摘要层 | 用真实分析任务和 error taxonomy 做更细的 agent/reasoning 评估 |
| 多模态对齐的收益边界不清 | “文本/视觉有用”与“何时有用”不是一回事 | 做 modality ablation、domain transfer、missing modality 研究 |
| 工业落地缺少公开、可复现的 pipeline 报告 | 工业论文多强调前景与架构 | 补 latency、维护成本、失败模式、human-in-the-loop 设计 |
| 数据治理仍是瓶颈 | 大模型结果高度依赖 corpus 质量 | 强化去重、泄漏检测、source provenance 和 benchmark auditing |

综合来看，LLM 在时间序列中的应用正在从“模型迁移问题”演化为“时序基础设施问题”。未来更有价值的工作，未必是再提出一个更大的模型，而是把数据治理、任务统一、reasoning interface、以及 domain workflow 真正打通。[Li, 2024, NMGEYFXY] [Li, 2025, 4A7VQ9KI] [Meyer, 2025, U72EAEL6] [Zhao, 2025, X8EVUCD7] [Ye, 2025, GS84W4SF]

## Traceability Table

| Claim | evidence_source | Traceable evidence |
|---|---|---|
| forecasting 仍是该 collection 的主任务中心 | `metadata+abstract` | [Ansari, 2024, 3WJZAR2B], [Shi, 2024, E3RMDSXY], [Goswami, 2024, ANF55ZAR], [Xiao, 2025, 6WFWZ84E], [Liu, 2026, PC45HAI6] |
| 研究主线正在从 LLM 适配走向原生 TSFM | `metadata+abstract` | [Jin, 2024, GNTDNEUI], [Liu, 2024, 5XCFMRIS], [Ansari, 2024, 3WJZAR2B], [Shi, 2024, E3RMDSXY], [Liu, 2026, PC45HAI6] |
| 多模态路线的核心价值是引入外部语义与场景知识 | `metadata+abstract` + `existing notes` | [Li, 2025, 9EER2W65], [Wang, 2024, F2ISE43Z], [Wu, 2025, GJI6A654], [Wang, 2025, GQ3FPJYV] |
| reasoning/agent 是 2025 之后的新增长点 | `metadata+abstract` | [Liu, 2025, 2H6GBXU3], [Guan, 2026, F6Z5YJ9B], [Zhao, 2025, X8EVUCD7], [Ye, 2025, GS84W4SF], [Kong, 2025, 2U932HE2] |
| benchmark 和数据治理正在变成基础设施议题 | `metadata+abstract` | [Li, 2024, NMGEYFXY], [Li, 2025, 4A7VQ9KI], [Zhou, 2025, K7RNQ6CX], [Meyer, 2025, U72EAEL6] |
| 工业方向已经从概念讨论转向体系化落地探索，但证据仍偏摘要级 | `metadata+abstract` + `[需确认]` | [Ren, 2025, C4RMX6WL], [Ren, 2025, IQ2AIFVZ], [Wang, 2025, 9WTUALLC], [Wang, 2025, VNFDCIHS], [陈致蓬, 2025, EGUTY2KE] |

## References

- [Jin, 2024, GNTDNEUI] Time-LLM: Time Series Forecasting by Reprogramming Large Language Models.
- [Tan, 2024, TGW8IKBF] Are Language Models Actually Useful for Time Series Forecasting?
- [Liu, 2024, 5XCFMRIS] CALF: Aligning LLMs for Time Series Forecasting via Cross-modal Fine-Tuning.
- [Tang, 2025, C6DH7KKX] LLM-PS: Empowering Large Language Models for Time Series Forecasting with Temporal Patterns and Semantics.
- [Ansari, 2024, 3WJZAR2B] Chronos: Learning the Language of Time Series.
- [Shi, 2024, E3RMDSXY] Time-MoE: Billion-Scale Time Series Foundation Models with Mixture of Experts.
- [Goswami, 2024, ANF55ZAR] MOMENT: A Family of Open Time-series Foundation Models.
- [Liu, 2024, 7U722X5F] Time-MMD: A New Multi-Domain Multimodal Dataset for Time Series Analysis.
- [Liu, 2026, PC45HAI6] Timer-S1: A Billion-Scale Time Series Foundation Model with Serial Scaling.
- [Xiao, 2025, 6WFWZ84E] TimeFound: A Foundation Model for Time Series Forecasting.
- [Li, 2025, 9EER2W65] Language in the Flow of Time: Time-Series-Paired Texts Weaved into a Unified Temporal Narrative.
- [Wang, 2024, F2ISE43Z] ChatTime: A Unified Multimodal Time Series Foundation Model Bridging Numerical and Textual Data.
- [Zhong, 2025, J36ULJXY] Time-VLM: Exploring Multimodal Vision-Language Models for Augmented Time Series Forecasting.
- [Wu, 2025, GJI6A654] Aurora: Towards Universal Generative Multimodal Time Series Forecasting.
- [Wang, 2025, GQ3FPJYV] ITFormer: Bridging Time Series and Natural Language for Multi-Modal QA with Large-Scale Multitask Dataset.
- [Liu, 2025, 2H6GBXU3] Time-R1: Towards Comprehensive Temporal Reasoning in LLMs.
- [Cheng, 2025, 4752UGGC] Can Slow-thinking LLMs Reason Over Time? Empirical Studies in Time Series Forecasting.
- [Zhang, 2025, EDNTT2WX] TimeMaster: Training Time-Series Multimodal LLMs to Reason via Reinforcement Learning.
- [Guo, 2026, EMKUFPMT] T-LLM: Teaching Large Language Models to Forecast Time Series via Temporal Distillation.
- [Guan, 2026, F6Z5YJ9B] TimeOmni-1: Incentivizing Complex Reasoning with Time Series in Large Language Models.
- [Zhao, 2025, X8EVUCD7] TimeSeriesScientist: A General-Purpose AI Agent for Time Series Analysis.
- [Ye, 2025, GS84W4SF] TS-reasoner: domain-oriented time series inference agents for reasoning and automated analysis.
- [Kong, 2025, 2U932HE2] Time-MQA: Time Series Multi-Task Question Answering with Context Enhancement.
- [Li, 2024, NMGEYFXY] FoundTS: Comprehensive and Unified Benchmarking of Foundation Models for Time Series Forecasting.
- [Li, 2025, 4A7VQ9KI] TSFM-Bench: A Comprehensive and Unified Benchmark of Foundation Models for Time Series Forecasting.
- [Zhou, 2025, K7RNQ6CX] CaTS-Bench: Can Language Models Describe Time Series?
- [Wen, 2024, BAGP3UMG] Measuring Pre-training Data Quality without Labels for Time Series Foundation Models.
- [Yao, , XUZQVJG8] Towards Neural Scaling Laws for Time Series Foundation Models.
- [Meyer, 2025, U72EAEL6] Rethinking Evaluation in the Era of Time Series Foundation Models: (Un)known Information Leakage Challenges.
- [Ren, 2025, C4RMX6WL] Industrial Foundation Model.
- [Ren, 2025, IQ2AIFVZ] Foundation Models for the Process Industry: Challenges and Opportunities.
- [Wang, 2025, 9WTUALLC] MetaIndux-TS: Frequency-Aware AIGC Foundation Model for Industrial Time Series.
- [Wang, 2025, VNFDCIHS] CoLLM: Industrial Large-Small Model Collaboration with Fuzzy Decision-making Agent and Self-Reflection.
- [陈致蓬, 2025, EGUTY2KE] 工业垂域具身智控大模型构建新范式探索.
