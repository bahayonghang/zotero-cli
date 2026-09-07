# 本地验证记录

检查日期：2026-09-07，Windows / PowerShell。基线：dev，HEAD `5b53e7f5c2dc488160bd2ddf1ba614776403b1d0`，开始时工作区干净。
仅新增本次 Trellis 规划与审查材料；未修改 Rust、工作流、正式说明或 skill。

## 环境与命令

Rust/Cargo 1.98.0；just 1.58.0（CI 固定 1.57.0）；Python 3.14.7；Node 26.7.0（docs workflow 为 Node 20）。
命令通过 RTK proxy 执行时保留原命令语义及退出码。未安装 `zot`，诊断路径固定为 `cargo run -q -p zot-cli -- ...`。

| 检查 | 结果 | 证据与限制 |
| --- | --- | --- |
| `cargo run -q -p zot-cli -- --json doctor` | PASS，exit 0，JSON ok=true | 能力细节见下；命令成功不代表所有能力可用 |
| `just ci` | PASS，exit 0 | fmt、locked check、clippy -D warnings、Rust 测试、skill 检查、Python 测试全部通过 |
| Rust workspace 测试 | PASS，277 个不同测试，0 fail/ignored | zot-cli 126；JSON integration 4；version guard 2；core 13；desktop 11；local 61；search integration 8；semantic integration 8；remote 44。version-check 提前重复执行 2 个 guard，不重复计数；4 个 doc-test target 各 0 个测试 |
| `python -m unittest discover -s scripts/tests -p 'test_*.py'` | PASS，5 个测试 | 单独执行与 just ci 内执行均通过，不累计为 10 个 |
| `actionlint .github/workflows/ci.yml .github/workflows/deploy-docs.yml` | PASS，exit 0 | 只证明语法和静态规则，不证明 GitHub 运行成功 |
| `cargo audit --json` | PASS，exit 0 | 305 个锁定依赖；0 vulnerabilities/warnings；advisory DB commit 5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5，数据库更新时间 2026-09-02 |
| `cargo deny check` | PASS with warnings，exit 0 | advisories/bans/licenses/sources 均 ok；存在 duplicate dependency 警告，不能写为完全无警告，也不据此升级依赖 |
| 显式检查两个 zot-brainstorm 镜像 | PASS，exit 0 | `python scripts/check_skill_mirrors.py --canonical skills/zot-brainstorm --mirror .agents/skills/zot-brainstorm --mirror .claude/skills/zot-brainstorm` |
| `npm --prefix docs run build` | BLOCKED，exit 1 | vitepress not recognized；docs/node_modules 不存在，尚未运行到文档编译。未擅自 npm ci / 安装依赖 |
| MSRV 1.85 / pinned nightly udeps | NOT RUN locally | rustup toolchain list 中无 1.85.0 或指定 nightly；不能以当前 stable 通过代替 |
| cargo machete / cargo udeps | NOT RUN locally | --version 探测报 no such command；没有安装工具 |

`just ci` 原始输出已保存为本任务的 [just-ci.log](just-ci.log)，临时副本位于 `%TEMP%/zot-evergreen-20260907-just-ci.log`；以上摘要为随任务保存的证据。未将私有 Zotero 内容或凭据写入报告。

## Harness 版本只读探测

本轮随后执行各命令的 --version，均 exit 0：Claude Code 2.1.263；codex-cli 0.153.4；grok 1.0.22（8f40483ca2a5）；Kimi Code 0.41.0；OMP 18.1.12。
这只确认 PATH 上的 CLI 身份/版本，不代表新会话已加载规则、skill、hook、subagent 或成功认证。未启动新的模型会话。

## doctor 能力详情

- 本地 SQLite 可读（schema 129）；没有查询或导出用户条目正文。
- connector / Local HTTP：connector-unreachable；提示启动 Zotero。未启动 UI、未导入。
- Web API：credentials-only 检查，configured=false、verified=false；不能声称完成写入验证。
- PDF：available=false、cached=false；首次 PDF 读取可触发下载，本轮没有下载或提取。
- embedding 未配置；存在本地索引不等于语义检索可用。
- doctor 提示旧 desktop_bridge / write_backend 配置，但本轮不修改用户配置。
- 当前实际 config_file 位于 Windows Roaming/zot/config.toml，与 AGENTS.md:45 的 Linux 固定路径写法不一致；实现依据为 src/zot-core/src/config.rs:184。

## 已复现的镜像门禁漏检

真实工作区的两个 skill 镜像目前都一致。漏检复现只发生在临时目录，没有改动真实 skill：

1. 建立 skills/zot、skills/zot-brainstorm 和 .agents/skills、.claude/skills 下相同的 SKILL.md。
2. 仅把 .agents/skills/zot-brainstorm/SKILL.md 改为不同内容。
3. 使用真实脚本绝对路径，在该临时目录执行默认 `--skip-if-all-missing`：exit 0，仅输出两个 zot 镜像一致。
4. 显式传 `--canonical skills/zot-brainstorm --mirror .agents/skills/zot-brainstorm`：exit 1，报告 content drift。

根因：justfile:62 安装遍历 skills 下所有目录；scripts/check_skill_mirrors.py:51 与 scripts/check_skill_mirrors.py:63 默认只检查 zot；justfile:75 没有扩大范围。
这证明检查入口覆盖不足，不证明 compare_trees 内容比较算法有错。

## 不构成缺陷的观察

- offline cargo metadata 中 wasip2/wit-bindgen 声明 Rust 1.87，但它们是目标相关依赖；未验证宿主是否选用，不能直接宣布 Linux MSRV 失败。
- CONTEXT.md / docs/adr 缺失按 docs/agents/domain.md 为允许的懒创建，不列改造。
- 当前 stable 测试没有失败；不得把历史 CI 失败冒充当前单元测试失败。
