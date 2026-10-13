# Zotero 本地内容操作能力调研与规划

## Goal

让 agent 通过 zot 整理 Zotero 桌面端已有条目：在同一 library 内添加、移除或移动分类归属，并手动补充 Info 面板字段及作者。无需配置 zotero.org Web API key，也无需安装自研插件，修改在桌面端可见。

用户最初要求结合网络搜索创建 Trellis 任务并分析可行性；调研和规划完成后，用户于 2026-10-13 明确“批准实施”。实施不包含修改真实主库文献。2026-10-13 用户已选择首版范围为“分类移动＋字段/作者补充”，没有选择标签/笔记或联网自动补全。

## Background

- CLI 并非完全不能写：现有 collection mutation、item update、notes、tags 等都调用 Web API。`src/zot-cli/src/context.rs:70` 校验 Web 凭据；`src/zot-cli/src/commands/collection.rs:9` 分发写操作；`src/zot-cli/src/commands/item/write.rs:124` 更新字段。
- 本机 `zot 1.0.1` 的 doctor 报告 Zotero 10.0.6、Local HTTP/Connector 可达、Web 凭据缺失；SQLite snapshot 本次报 `zotero-db-busy`。额外只读 GET 已确认 Local API v3、schema 44、Server ID 存在。真实写入未验证。
- 官方 Zotero 10 已增加 Local API 写入。仓库当前“Local HTTP 只读”的产品边界源于更早版本；接入新能力需要显式修改相关契约。SQLite 仍然只读，Connector 仍然 import-only。
- 当前 `--field` 只生成字符串，无法正确表达 creators 数组；没有独立 collection move 命令。证据和官方链接见 [调研报告](research/findings.md)。

## In Scope / Requirements

| ID | 首版要求 |
| --- | --- |
| R1 | 接入 Zotero 10+ 官方 Local API；明确选择 local 写路径，支持本地 user library 与已存在、可写的 group library；不依赖 Web 凭据。 |
| R2 | 同库顶层普通条目支持分类添加、移除、A→B 移动。一次移动保留其他分类、item key、children、附件和笔记；不复制条目。 |
| R3 | 按当前 itemType/schema 补充字符串字段，覆盖 title/date/DOI/url/abstractNote/期刊或会议字段等；默认仅填空，覆盖非空值须显式指定。 |
| R4 | 支持结构化 creators 输入，保留姓名形式、creatorType 和顺序。空列表可补全，替换已有列表需显式覆盖并展示完整差异。 |
| R5 | 本地 mutation 默认预览；确认绑定同一 Server ID、library、item key 和预览版本。发生并发编辑、实例切换、只读目标或无权限时拒绝写入。 |
| R6 | 写后通过同一 Local API 回读，报告预览/无变化/成功/冲突/失败/结果未知；不把连接超时或 API 仅可达当作写成功。 |
| R7 | 保持既有 Web 写命令兼容；local 失败不转 Web/Connector/SQLite。doctor 区分版本支持、可达、授权和实际验证，普通诊断不弹授权框。 |
| R8 | 更新 canonical skill、必要镜像、双语使用文档与相关 spec/AGENTS 边界；agent 自然语言流程能发现并选择 local 写路径。 |

用户已明确批准最终方案，按默认预览、显式覆盖、旧 Web 默认行为兼容策略实施。

## Acceptance Criteria

调研交付：

- [x] A0：提供仓库现状、2026-10-13 官方网络证据、版本条件、本机只读验证、路线比较及完整能力矩阵；见 `research/findings.md`。
- [x] A1：记录用户选择，完成 PRD、design、implement；调研阶段未修改产品代码、Zotero 内容或授权状态。后续已获批准并完成代码实现。

实现验收（自动化与真实写入证据分别记录于 validation.md；组库权限和提交中断连仍为模拟证据）：

- [x] AC1 / R1,R7：无 Web key 时，Zotero 10+ 可完成首版本地操作；旧版本、不支持或关闭本地 API 时给出准确原因；绝不自动换后端。
- [x] AC2 / R2：条目属于 A、C，移动 A→B 后恰为 B、C；item key、元数据和 children 不变。重复操作、源不存在、A=B、目标已含条目均有确定结果。
- [x] AC3 / R3：空 DOI、摘要、会议名等可补齐；非空字段默认保留并报告，显式覆盖后只改指定字段；无效字段/无效值在授权前失败。
- [x] AC4 / R4：支持多位作者和机构作者，保持顺序及角色；不得把 JSON 数组写成字符串或隐式丢掉已有作者。
- [x] AC5 / R5：预览不请求授权、不写库；旧版本、错误 Server ID、并发编辑、跨库目标均不能写；拒绝授权后没有其他写请求。
- [x] AC6 / R6：成功由响应和回读共同支持；网络断开后不盲重试。回读失败要区分“服务器已接受但验证失败”与“提交结果未知”。
- [x] AC7 / R1,R5：本地操作不要求 SQLite snapshot 成功；目标数据及版本来自 Local API，Web/SQLite 版本不参与本地写前置条件。
- [x] AC8 / R7,R8：现有 Web/Connector 命令回归通过；JSON 无密钥，doctor 无授权副作用；文档准确标注 Zotero 10+ 能力与旧版限制。
- [x] AC9：相应 crate 测试、JSON 合约测试、`just ci` 通过；在独立测试库完成真实授权、移动、字段/作者修改和回读验证，不以 mock 代替真实 UI 验收。

## Out of Scope

首版不接入本地 tags、notes、附件上传、annotation、saved-search、collection 新建/改层级/删除、trash/restore、merge/dedupe、跨库复制/迁移、批量筛选写入、itemType 转换或联网自动补全。它们的可行性已纳入调研，不属于实现承诺。

不升级 Zotero、不恢复旧插件桥、不自动切换后端、不直接写 `zotero.sqlite`，不将截图中的内容或猜测的 DOI 写入真实库。现有 SQLite busy 问题独立记录，不扩展为本轮修复。

## Decisions and Artifact Status

- 用户已确认功能范围并批准实施；实现依据见 `design.md`。
- 已实测本机授权、字段/作者/分类写入、UI 显示、并发编辑拒绝和重启复用。真实组库权限、提交中断连、Undo/BBT 及其他平台仍列为验证限制。
- 代码、自动化检查及 AC9 独立测试库真实授权/写入/UI 验收已完成；见 validation.md 和 acceptance/results.json。
- 实现已提交为 `16ce140`；验收材料进入 Phase 3.4 提交与归档流程。主库已恢复，未创建 GitHub issue、推送或修改主库文献。
