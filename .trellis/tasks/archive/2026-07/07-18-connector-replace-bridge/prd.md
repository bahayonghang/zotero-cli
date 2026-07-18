# Replace zot-bridge plugin with Zotero built-in connector/local API

## Goal

去掉自研 `zot-bridge` XPI 插件这条本机写入通道,改用 Zotero 桌面端**内置**的两个 HTTP 面
(`http://127.0.0.1:23119` 上的 `/connector/*` 和 `/api/*`),对齐 Codex 官方 zotero skill
(`ref/zotero/skills/zotero`)的连接方式:只要 Zotero 应用在运行即可,无需安装/配对任何插件。

## Background / 分析结论

Codex 官方 skill 不需要插件的原因:

1. **Local API**(`/api/users/0/...`,Zotero 7+ 内置):只读实现 Web API v3,无需 API key,
   由 pref `extensions.zotero.httpServer.localAPI.enabled` 控制。
2. **Connector server**(`/connector/*`,始终随 Zotero 启动):浏览器 connector 用的端点,
   支持 `POST /connector/import`(BibTeX/RIS 导入到当前选中 collection)、`saveItems`、
   `getSelectedCollection`。这是内置的**本机写入**通道,但只能"新增/导入",不能改已有条目。

当前项目的 `zot-bridge` 插件(`plugins/zot-bridge/`,`src/zot-desktop` 的 bridge client)
唯一实际能力是本机 merge preview/apply(`item merge` / `library duplicates-merge` /
`library dedupe` 的 desktop 后端),外加 pair/token 生命周期。

**关键 tradeoff(已知能力损失)**:Zotero 内置面做不了 merge(local API 只读,connector
只能新增)。删除插件后,merge/dedupe 只能走 Zotero Web API(现有 `WebMergeWriter` 路径),
"无 API key 的本机去重合并"这一能力被放弃。换来的是:零插件安装、零配对、零协议维护。

## Requirements

- 新增 connector 本机写入路径(导入 BibTeX/RIS、读取选中 collection)。→ 子任务 connector-local-write
- 删除 zot-bridge 插件、bridge 命令组、desktop 写后端及全部配对/token 机制。→ 子任务 remove-bridge-plugin
- 按 Codex skill 的功能面重写 `skills/zot/SKILL.md`。→ 子任务 zot-skill-connector-update

## Task Map(子任务与顺序)

1. `07-18-connector-local-write` — 先加能力:zot-desktop 改造为 connector client,
   新 CLI 命令 + doctor 能力位。不动 bridge 代码,保证期间 CLI 始终可用。
2. `07-18-remove-bridge-plugin` — 后删旧路:插件目录、bridge 命令、DesktopMergeWriter、
   `WriteBackend::Desktop`、config `desktop_bridge`、doctor `desktop_write`、相关 spec/docs。
   依赖子任务 1 完成(doctor 能力位重排一次到位)。
3. `07-18-zot-skill-connector-update` — 最后改 skill:必须在 1、2 落地后执行,
   保证 skill 描述与 CLI 真实命令面一致。

## Cross-child Acceptance Criteria

- [ ] `just ci` 全绿 + `npm --prefix docs run build` 通过 + `git diff --check` 干净;仓库无 `zot-bridge` 插件代码/XPI 逻辑,`just --list` 无 plugin-test/xpi-check
- [ ] 只开 Zotero(不装任何插件)即可:`zot --json doctor` 报告 `connector_write` 可用;本机导入 BibTeX/RIS 成功;选中只读目标被拒
- [ ] merge/dedupe 全量走 Web API 后端,错误信息不再引用 bridge/pair;`zot bridge` / `config set write_backend` 均不存在
- [ ] envelope 移除 `write_backend`/`selected_write_backend`/`desktop_write`,`api_version` 仍为 1,`docs/agents/limits.md` 无矛盾
- [ ] 旧配置残留 `desktop_bridge` 段**或**仅 `write_backend="desktop"` 时,CLI 不崩溃并给出迁移提示(不读/不打印 token)
- [ ] `skills/zot/` 三份资产(SKILL.md 含 frontmatter、evals.json、test-prompts.json)与最终命令面零偏差,`just skills-check` 通过
- [ ] README(中英)/ CHANGELOG / 双语 VitePress docs / `.trellis/spec` 中 bridge 文档同步更新

## Decisions(已定,供集成验收对齐)

- **envelope schema**:删除 `write_backend`/`selected_write_backend`/`desktop_write` 字段,
  `meta.api_version` **保持 1**(用户 2026-07-18 决定「全删且不升版本」),并同步修订
  `docs/agents/limits.md` 的截止标记措辞,消除文档矛盾。
- **确认门**:connector `item import` 用项目既有 `--confirm`,不引入 ref 的 `--yes`。
- **能力取舍**:本机 merge/dedupe(原 desktop-only)随 bridge 移除,统一改走 Web API。

## Notes

- 本 parent 任务只承担需求集、任务地图与最终集成验收,不直接作为实现目标。
- 本次规划已按 Codex 审阅(2026-07-18,7 条 P1/P2)逐条核实并落到子任务;审阅结论全部属实。
- 参考实现:`ref/zotero/skills/zotero/scripts/zotero.py`(stdlib-only,865 行)与
  `ref/zotero/skills/zotero/references/local-api-routes.md`(路由清单)。
