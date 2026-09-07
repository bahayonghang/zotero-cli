# Workflow / CI / MSRV 独立审查

审查日期：2026-09-07。基线为 `dev` / `origin/dev` 的
`5b53e7f5c2dc488160bd2ddf1ba614776403b1d0`；`origin/main` 为合并提交
`5b368d073021bdca67672353ae25b25ef2375a40`。本报告仅做只读审查和规划，未运行
Cargo 构建/测试、未改实现或正式规则、未发起或重跑 GitHub workflow、未改环境保护策略。

## 结构与现有门禁

- 根 `Cargo.toml:1-8` 定义 5-crate workspace；`Cargo.toml:11-17` 统一版本、edition 2024
  和 Rust 1.85；`Cargo.toml:63-70` 禁止 `unsafe`、`dbg!`、`todo!`、`unwrap()`。
- `src/zot-cli/src/main.rs:20-54` 负责参数解析、上下文建立、协议校验和统一错误出口；
  `src/zot-cli/src/commands/mod.rs:19-38` 把命令分发至 doctor/config/library/item/collection/
  graph/workspace/sync/mcp。CLI 上层同时依赖 `zot-core`、`zot-local`、`zot-desktop`、
  `zot-remote`（`src/zot-cli/src/context.rs:5-9`），所以入口层回归可能跨本地 SQLite、
  connector 和 Web API 三条边界。
- `justfile:9-25` 把 version guard、locked check、fmt、clippy、workspace tests 拆成纯检查；
  `justfile:74-80` 组合 skill mirror 和 Python tests 为 `just ci`。
- `.github/workflows/ci.yml:14-44` 在 Linux/Windows/macOS stable 上运行 `just ci-check`
  并断言无生成漂移；`:46-62` 只在 Ubuntu 验证 Rust 1.85；`:64-129` 另跑 audit/deny、
  machete 和固定 nightly udeps。
- 文档发布是独立面：`.github/workflows/deploy-docs.yml:6-9` 在 published release 或手动
  dispatch 时触发，`:21-54` 构建并上传 VitePress artifact，`:56-66` 部署到受保护的
  `github-pages` environment。它不属于当前 `just ci` 或 PR CI。

## Findings（按优先级）

### P1 / HIGH：release tag 约定与 Pages 环境策略冲突，正式发布路径已真实失败

证据链：

1. workflow 声明“每个 published release”自动发布（`.github/workflows/deploy-docs.yml:4-9`），
   README 也只笼统承诺通过该 workflow 发布（`README.md:199`、`README.zh-CN.md:199`）。
2. 最新 release 使用 tag `1.0.0`。run
   [29644116256](https://github.com/bahayonghang/zotero-cli/actions/runs/29644116256)，SHA
   `dfc672bfcea344260c03abbcb5ef213edd116407`：`Build VitePress site` 所有步骤成功，
   `Deploy to GitHub Pages` 在 runner 分配前失败（0 steps）。check-run annotation 明确为
   `Tag "1.0.0" is not allowed to deploy to github-pages due to environment protection rules.`
3. GitHub API 当前只读结果：`github-pages` 使用 custom branch policy，仅允许 branch `main`
   和 tag `v*`。同一 SHA 随后的手动 main run
   [29644167450](https://github.com/bahayonghang/zotero-cli/actions/runs/29644167450) 构建、部署均成功。

因此根因不是 VitePress、artifact 或 Pages 权限，而是发布 tag `1.0.0` 不匹配 `v*`。
该问题尚未被仓库内契约修复；未来再次发布不带 `v` 的 tag 会重现。

最小改造：以后统一使用 `vX.Y.Z`，保留现有环境策略，不改旧 tag/release；在
`AGENTS.md` 的 release 段、`README.md`、`README.zh-CN.md` 明写该约定，并在
`.github/workflows/deploy-docs.yml` 的 release 构建前增加可读的 tag preflight，使错误在
安装/构建前指向约定。适用于五套 harness，因为它是仓库发布契约，不依赖某个客户端。

必须通过：`actionlint .github/workflows/ci.yml .github/workflows/deploy-docs.yml`、`just ci`、
静态断言 release preflight 与文档一致。真正的 release-triggered Pages 成功仍是 hosted
`UNVERIFIED`，需要另一次真实发布授权；本规划不授权创建 tag/release、改环境或重跑 workflow。

分工：强模型（Codex 或 Claude Code）确定版本/tag/环境三方 source of truth 并审查失败链；
便宜模型可按批准文本修改 YAML/三份说明和跑静态/本地门禁；强模型复核 hosted run 与实际 SHA。

### P2 / MEDIUM：canonical skill 有两个，默认镜像门禁只检查 `zot`

`justfile:62-70` 安装时遍历 `skills/` 下所有目录并复制到 `.agents/skills` 与
`.claude/skills`；当前 canonical 实际包含 `zot` 和 `zot-brainstorm`。但
`scripts/check_skill_mirrors.py:51` 默认 canonical 固定为 `skills/zot`，`:63` 默认 mirror
也只固定到两个 `zot` 路径，`justfile:75` 没有补充 `zot-brainstorm` 参数。

已在临时目录用真实脚本复现：让 `zot` 镜像一致、只污染
`.agents/skills/zot-brainstorm/SKILL.md` 后，默认 `--skip-if-all-missing` exit 0；显式检查
`zot-brainstorm` exit 1。真实工作区两个 brainstorm 镜像当前一致，因此这是门禁覆盖缺陷，
不是当前生成物已经漂移。详见 `research/local-validation.md`。

最小改造：让 `scripts/check_skill_mirrors.py` 默认按 canonical root 枚举所有 skill，并把每个
skill 与两个 mirror root 的同名目录比较；保留 focused CLI 仅在测试/诊断确有价值时再保留。
更新 `scripts/tests/test_check_skill_mirrors.py`，覆盖“第二个 skill 漂移必失败”“全体 mirror
未安装可跳过”“只缺其中一个 skill 不可跳过”。`justfile:74-75` 只保留一个清晰入口。

必须通过：脚本 focused unittest；在临时 fixture 上先看到第二个 skill drift red、修复后 green；
真实两套 mirrors 显式一致；`just ci`；`git diff --check`。该任务属于确定性脚本改造，可由便宜
模型执行；强模型审查 canonical/mirror/缺失语义，防止 `--skip-if-all-missing` 把部分缺失吞掉。

### P2 / MEDIUM：当前 docs 源码没有对应 HEAD 的 PR/本地构建证据

`.github/workflows/ci.yml:3-8` 的 PR/push CI 不含 docs job；`justfile:30-32` 的 `docs` recipe
运行 `npm install` 后启动 dev server，不是 CI build。上一次 hosted VitePress build 是
`dfc672bf...`，而该 SHA 到当前 `5b53e7f...` 之间已有 9 个 `docs/**/*.md` 文件变更；当前
HEAD 的成功 Rust CI run 不会验证这些页面。

本机 `npm --prefix docs run build` exit 1，原因是 `docs/node_modules` 不存在、`vitepress` 未识别，
命令尚未进入文档编译；没有安装依赖，因此不能把它记为源码失败。`docs/package.json:5-11`
已有确定的 `build` script，`docs/package-lock.json:2474-2475` 锁定 VitePress 1.6.4。

最小改造：在 `justfile` 增加非 dev 的 `docs-check`（锁文件安装 + build），并在
`.github/workflows/ci.yml` 增加 Ubuntu/Node 20 docs job；是否把它并入默认 `just ci`，由强模型
根据“Rust 开发者是否必须安装 Node”这一成本决定，但 PR 必须有 hosted docs gate。

必须通过：`npm --prefix docs ci`、`npm --prefix docs run build`（或批准后的等价 `just docs-check`）、
`actionlint`、`just ci`、新 PR SHA 上 docs job green。执行 YAML/recipe 可给便宜模型；强模型审查
本地 gate 与 hosted gate 是否一致，以及中英文链接/构建产物是否真实覆盖。

### P2 / MEDIUM：Rust 1.85 证据只覆盖 Linux，Windows/macOS 的 MSRV 承诺未验证

所有 crate 都继承 `Cargo.toml:17` 的 `rust-version = "1.85"`，stable job 明确支持三系统
（`.github/workflows/ci.yml:14-20`），但 MSRV job 固定 `ubuntu-latest`
（`.github/workflows/ci.yml:46-62`）。当前 HEAD 的 run
[30210865785](https://github.com/bahayonghang/zotero-cli/actions/runs/30210865785) 证明 Ubuntu
1.85 locked workspace 成功；本机未安装 1.85，Windows/macOS 的 1.85 均为 `UNVERIFIED`。
offline metadata 看到的 `wasip2`/`wit-bindgen` Rust 1.87 是 target-dependent，不能据此宣布
宿主 MSRV 失败。

最小改造的决策门：若 1.85 是三平台承诺，把 msrv 改为三系统 matrix；若只承诺 Linux，必须在
`AGENTS.md`/README 明写范围。当前项目已把 stable gate 做成三平台，优先建议三平台 MSRV matrix。

必须通过：三平台 hosted `cargo +1.85.0 check --workspace --locked`，且 stable matrix 与现有
`just ci-check` 继续 green。YAML 执行可交便宜模型；强模型负责判断平台承诺、审查 target-specific
依赖和 hosted 证据，不能用当前 stable 代替 1.85。

### P3 / LOW：GitHub Actions 更新积压存在，但不能把旧 PR 的失败当成当前 HEAD 失败

当前 workflows 仍使用 `actions/checkout@v4` 等 major tag；历史 runner annotation 已警告 Node 20
action runtime 迁移。Dependabot 的 Actions PR #1-#4 自 2026-04 保持 open，均基于旧工程基线并
显示旧 CI 失败；其日志/失败上下文不等于这些 action major 与当前 HEAD 不兼容。当前 HEAD 的 July
run 已在 GitHub 强制 Node 24 的 runner 上成功，但没有 2026-09 的 fresh hosted run。

最小改造：不要直接合并旧 PR；基于当前 HEAD 逐个刷新/重建 Actions 升级，先 checkout，再 Pages
actions，每次用 actionlint、现有 CI 和（涉及 Pages 时）manual dispatch 验证。远端关闭、rebase、
rerun、merge 都需要单独授权。便宜模型可做单个版本替换；强模型审查 action changelog、权限和
hosted job，不把“旧 PR 红”解释为升级根因。

## 当前与历史 workflow 结果

| 状态 | Run / SHA | 结果和根因 |
| --- | --- | --- |
| PASS（当前 dev HEAD） | [30210865785](https://github.com/bahayonghang/zotero-cli/actions/runs/30210865785) / `5b53e7f...` | 7 个 job 全绿：stable 三平台、Ubuntu MSRV 1.85、security、machete、udeps。 |
| PASS（当前 main merge） | [30211159572](https://github.com/bahayonghang/zotero-cli/actions/runs/30211159572) / `5b368d0...` | main push CI 全绿；该 SHA 合并了当前 dev HEAD。 |
| FAIL→RESOLVED | [30210314607](https://github.com/bahayonghang/zotero-cli/actions/runs/30210314607) / `4cd1f20...` | stable 三平台在测试后因 `.agents/skills/zot` 不存在失败；nightly job 因把日期当 action ref 而 setup 失败。后续 `5b53e7f` 仅在 workflow/justfile 上修为 `uses: ...@nightly` + `toolchain:` 以及 `--skip-if-all-missing`，同 HEAD run 全绿。 |
| FAIL→RESOLVED（更早旧 workflow） | [28945565328](https://github.com/bahayonghang/zotero-cli/actions/runs/28945565328) / `cd39ab4...` | `cargo fmt --check` 对多处 Rust 文件报 diff；后续 main run `29146804767` 成功。不是当前测试失败。 |
| FAIL（当前发布契约仍可复现） | [29644116256](https://github.com/bahayonghang/zotero-cli/actions/runs/29644116256) / `dfc672b...` | release tag `1.0.0` 不匹配 environment 允许的 `v*`；build 成功、deploy 0 steps。 |
| UNVERIFIED（旧 open PR） | PR #10 / `581e9be...` | pdfium-render 0.9.1 的 check/clippy/test 曾 exit 101，但 run logs 已 HTTP 410，只剩通用 annotation；不得编造依赖失败根因，也不得外推到当前锁文件。 |

## 本轮验证分类

PASS：当前 `just ci`（277 个不同 Rust 测试 + 5 个 Python 测试）、actionlint、cargo audit、
cargo deny，以及当前 HEAD 的 7-job hosted CI；详见 `research/local-validation.md`。

FAIL：没有当前 HEAD Rust/test 失败。确认的仍有效失败是 release-tag / Pages policy 契约；确认的
门禁红能力是临时 fixture 中 `zot-brainstorm` drift 显式检查 exit 1。

SKIPPED / BLOCKED：本机 docs build 因依赖未安装未进入编译；未安装 Rust 1.85、固定 nightly、
cargo-machete、cargo-udeps，因此没有擅自安装或用当前 stable 冒充。

UNVERIFIED：当前 HEAD docs build、Windows/macOS Rust 1.85、真实 Web API 写入、connector、PDF、
embedding，以及修复后下一次真实 release-triggered Pages deploy。上述缺证不能写成成功。
