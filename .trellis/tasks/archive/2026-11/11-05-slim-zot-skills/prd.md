# 瘦身 zot 与 zot-brainstorm skill 入口

## Goal

Agent 触发 `zot` 或 `zot-brainstorm` 时，只加载路由、安全内核和输出契约；当前任务的命令细节再从同一 skill 包按需读取。两条 description 要能区分库操作与文献综合，且不改变已有读写安全行为。

## Background

Canonical 包是 `skills/zot` 与 `skills/zot-brainstorm`。`just skills-sync` 复制整个 skill 目录到 `.agents/skills` 和 `.claude/skills`；`just skills-check` 比较整棵相对文件树和字节。同包 `references/` 会随镜像发布，不需要改镜像机制。

`skills/zot/SKILL.md` 现有 613 行、约 23.8 KB。始终加载的内容包括十个意图桶、Zotero 10+ Local API 的 7a–7d 细节、诊断字段清单、三层安全门、23 条语言映射，以及 `ref/zotero-cli` 和 `ref/zotagent` 迁移表。

`skills/zot-brainstorm/SKILL.md` 现有 169 行、约 8.3 KB。它混合了必读流程、来源解析命令、证据等级和 HTML 约束；报告正文模板已经在 `templates/`。

`docs/skills/` 与 `docs/en/skills/` 是给人看的说明，不是 agent 的按需执行参考。

`skills/zot` 的 `evals/evals.json` 与 `test-prompts.json` 各有 46 条且 prompt 逐条相同。`zot-brainstorm` 两份各有 12 条，但场景不完全相同：歧义 collection 名和默认保存路径只在 test prompts；离线 HTML 模板和伪造 `zot brainstorm` 命令只在 evals。这些是静态夹具，不是已执行的模型评测。

现有 `zot` description 约 524 字符，功能清单与 `zot-brainstorm` 重叠，且没有把缺陷分析、创新点和研究报告交给姊妹 skill。`zot-brainstorm` description 只有英文，而回归样例主要是中文。两条都是单行，并以 `(v1.1.0)` 结尾；`justfile` 的 `version-sync` 只识别以 `description: ` 开头的单行。

## Requirements

### R1. 入口只保留必读内核

- `skills/zot/SKILL.md` 只保留触发边界、执行原则、意图分桶、调用顺序、诊断触发条件、写入安全内核、失败停止规则、输出契约，以及指向具体参考的指令。
- `skills/zot-brainstorm/SKILL.md` 只保留触发边界、只读默认值、输入类型、分析顺序、默认输出契约和失败停止规则，以及指向细节参考的指令。
- 每个入口不超过 250 行。不得为了达标删除安全停止条件。
- 每个移出的规则在入口保留一句可执行的路由或停止条件，并给出唯一相对链接。

### R2. 细节仍随 skill 包发布

- 命令细节、长示例、迁移表、诊断字段清单和 HTML 约束放入各自 skill 包的 `references/`。
- 不新增第二个可被安装器发现的 `SKILL.md`。
- 不把 agent 执行细节迁到 `docs/`，也不把用户教程复制回 skill 包。

### R3. 行为与安全不回归

- SQLite 只读。Zotero 10+ Local API 只覆盖已实现的字段、作者、分类成员、单条标签、分类创建和批量整理。Connector 仅新增导入 BibTeX/RIS。其余 mutation 走 Web API。
- 保留预览、确认、版本条件、批量写入、low-confidence 默认跳过、失败不跨路径 fallback，以及打开库的桌面读来源语义。
- `zot-brainstorm` 默认只读，结论保持证据等级和可追溯性。写入 Zotero、Obsidian 或外部系统仍需独立授权并交给 `zot` 安全门。
- 不新增、重命名或假装存在 CLI 命令；不改变 `zot mcp serve` 的未实现边界。

### R4. description 提高路由精度

- 重写两条 description，使库内查询、证据提取、workspace 维护和安全写入落到 `zot`，基于真实 Zotero 来源的缺陷分析、创新方向和研究报告落到 `zot-brainstorm`。
- 即使用户没有点名 skill，也要能按任务触发；泛化找论文、引用格式教学、非 Zotero PDF 和单篇摘要仍然排除。
- description 保持单行 `description: ... (vX.Y.Z)`，版本后缀继续由现有 `version-sync` 维护，不改成多行 YAML。
- 中文主请求和现有英文请求都要能从 description 找到关键区分词。

### R5. 回归资产与入口同步

- 更新两个 skill 的 eval 与 test prompt，使它们检查拆分后的路由和“细节在哪个参考文件”，而不是要求复述整份旧入口。
- `zot-brainstorm` 的两套样例覆盖同一组场景，保留歧义来源、默认路径、离线 HTML 和近邻负例。
- 更新仍把已移动内容说成全在根 `SKILL.md` 的仓库引用。用户文档只在这种失真时更新。
- 用 `just skills-sync` 生成镜像，再用 `just skills-check` 验证；不手工编辑镜像。

## Acceptance Criteria

- [ ] AC1：两个根 `SKILL.md` 都不超过 250 行，且仍包含触发、排除、路由、安全停止条件和输出契约。
- [ ] AC2：旧入口的每个行为规则都能在同一 skill 包的根文件或 `references/` 中找到，并且入口有唯一相对链接。
- [ ] AC3：现有 `zot` 样例的期望路由不反向变化；新增的近邻样例把文献综合报告交给 `zot-brainstorm`，而不是当成普通库操作。
- [ ] AC4：两条 description 都是单行，版本后缀等于 `Cargo.toml` 的 `workspace.package.version`，并满足 R4 的正向触发和排除条件。
- [ ] AC5：`zot-brainstorm` 的 eval 与 test prompt 覆盖同一有序场景集，包括歧义来源、默认路径、离线 HTML 和伪造命令。
- [ ] AC6：两个包内都没有根目录之外的 `SKILL.md`；入口里的相对链接都能解析到非空文件。
- [ ] AC7：`just skills-sync` 后两个管理镜像与 canonical 树字节一致，随后 `just skills-check` 通过；未手工修改镜像。

## Out of Scope

- 不改 Rust CLI 行为、命令名、写入后端或 API 契约。
- 不新增 skill、合并两个 skill，也不把任何一个发布到独立 GitHub 仓库。
- 不引入 Skill IR、发布 README、先例研究报告或外部安装验证。
- 不把现有 JSON 夹具升级成模型运行时评测器；未实际跑的评测保持为 missing evidence。
- 不重写 VitePress 站点，也不翻译 `zot-brainstorm` 正文。
- 不处理当前无关的 Dependabot 任务。

## Confirmed Decisions

- D1：只改变 skill 包的组织和触发描述，不改产品行为。
- D2：执行细节留在各自 `skills/<name>/references/`。
- D3：保留现有两个 skill 名和职责分工。
- D4：重写两条 description 以提高路由精度，但仍保持 `version-sync` 所依赖的单行版本后缀。
