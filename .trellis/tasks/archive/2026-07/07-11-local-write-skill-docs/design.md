# Design: 本机写入 skill 与文档对齐

## Skill Contract

`skills/zot/SKILL.md` 保持单一 operator skill，description 应同时覆盖：

- 查询/提取/整理本机 Zotero 内容；
- 通过 desktop bridge 安全修改本机库；
- 显式 Zotero Web API 写入；
- duplicate merge/dedupe、doctor、bridge setup/pair/status 和 workspace。

description 不承诺任意 local write；正文用 capability table 说明第一阶段实际范围。

## Decision Flow

```text
User request
  -> read-only?
       yes: local CLI path; no bridge/web credential gate
       no: run doctor
  -> requested backend explicit?
       desktop: require desktop_write available
       web: require web_write configured
       none: use effective profile write_backend
  -> command supported by selected backend?
       no: explain limitation; do not fallback
  -> preview / affected scope
  -> explicit confirmation
  -> execute and verify actual write_backend/result
```

skill 必须把 Local HTTP API 与 desktop bridge 分开：前者只读且无需 key，后者通过
插件访问 Zotero JS API。desktop token 不是 Zotero API key。

## Safety Copy

- `bridge setup` 只生成并打开 XPI，不代表安装完成。
- pair code 五分钟单次使用；不要让用户把 code/token 发进 issue、日志或 prompt。
- `item merge` / `duplicates-merge` / `dedupe` preview 是“尚未修改”。
- `library dedupe --confirm` 默认 normal-only；low groups 先展示并停下。只有用户明确
  要求承担 low-confidence 风险时才使用 include flag。
- backend 失败时说明原 backend、错误和恢复动作；不得建议自动 fallback。
- 直接 SQLite 写永远拒绝；只读 SQLite 路径可以继续用于 search/dedupe planning。

## Eval Set

在现有 26 条基础上至少增加以下案例：

| Case | Expected route | Key assertions |
| --- | --- | --- |
| no-key local dedupe | desktop | doctor/bridge, dry-run, no Web key, normal-only |
| explicit remote merge | web | `--write-backend web`, Web credentials, preview/confirm |
| plugin not installed | stop/setup | no web fallback, accurate setup hint |
| Zotero stopped | stop | no fallback, preserve library |
| local read near miss | local read | no bridge pairing or Web key |
| direct SQLite update | refuse | states read-only boundary |
| low-confidence batch | review/skip | never invent include flag |
| local tag request | unsupported desktop | do not claim Phase 1 support; offer explicit web only if user chooses |
| old unpaired profile | web default | explains compatibility and does not silently select desktop |

`test-prompts.json` 保持轻量人工 prompt/expected；`evals/evals.json` 保存完整 expectations。
安全断言应客观、可由 grader 判断，避免只写“回答得好”。

## Skill-Creator Evaluation Workflow

这是 existing skill improvement：实施前把 canonical `skills/zot` snapshot 到临时 workspace，
baseline 使用 old skill，不是 no-skill。新旧版本对同一 prompts 运行；保存 eval metadata、
timing、grading，聚合 benchmark，然后使用
`C:\Users\lyh\.skillsmanage\skills\skill-creator\eval-viewer\generate_review.py` 生成 viewer。

evaluation workspace 放 `%TEMP%\zot-skill-workspace\iteration-1`，不提交运行 transcript、
token 或 viewer output。用户审阅后才根据反馈改 skill；如需第二轮，viewer 传
`--previous-workspace`。trigger description optimization 只在行为稳定且用户认可后进行。

当前 Codex 工作流是 inline，但 skill eval 属独立评测；若运行环境允许且 skill-creator
要求，可使用并行 isolated eval agents。不得把 eval agent 当作 Trellis implement/check
sub-agent，也不得让其访问真实 Zotero token。

## Mirror Generation And Drift Check

保留 `just install` 的 canonical -> `.agents/.claude` 生成方向。新增
`scripts/check_skill_mirrors.py` 或等价结构化比较：

- 递归比较文件相对路径和 bytes；
- canonical 多/少文件、mirror extra file、内容差异均失败；
- 忽略规则只允许明确列出的 generated cache，不做模糊 glob；
- `just skills-check` 调用该脚本，并加入 `just ci`。

实施顺序是先改 canonical，再运行 generator 更新两份镜像，最后运行 check。禁止分别
手改三份以“修绿”。

## Documentation Map

根据真实页面更新：

- `README.md` / `README.zh-CN.md`：能力边界、quick start、desktop vs web。
- `docs/cli/config.md` / `docs/en/cli/config.md`：write_backend、bridge config 脱敏。
- `docs/cli/library.md` / `docs/en/cli/library.md`：dedupe backend、low-confidence。
- merge 所在 item 页面：preview/apply/backend output。
- skills safety/getting-started/architecture 页面：XPI、pairing、no fallback、SQLite refusal。

文档命令必须从 clap/help 或 tested examples 复制，不凭规划猜 surface。真实 token/code 用
`PAIR-CODE` / `TOKEN_REDACTED`，不要使用看似有效的 secret。

## Compatibility

保留现有 26 条 eval，除非命令行为正式变化；更新写入预期时保留其原始 intent。
skill description 和 docs 不能让只读 query 过度触发 bridge。旧 profile 的 web default、
explicit web 和 unsupported desktop mutation 都是防回归重点。
