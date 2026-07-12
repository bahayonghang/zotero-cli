# config 命令

`config` 用于查看和修改 `~/.config/zot/config.toml`。

它是运行时参考页，不是 agent 使用主入口。

## 子命令

```bash
zot --json config show
zot --json config init --library-id 123456 --api-key abcd
zot --json config init --target-profile work --library-id 123456 --api-key abcd --make-default
zot --json config set library-id 123456
zot --json config set api-key abcd --target-profile work
zot --json config set write-backend desktop
zot --json config profiles list
zot --json config profiles use work
```

## show

```bash
zot --json config show
zot --json --profile work config show
```

用途：

- 看当前有效配置
- 看默认 profile
- 看当前 session 选中了哪个 profile
- 排查 effective `write_backend`、desktop bridge、Web 写凭据、embedding、data dir 是否缺失

`desktop_bridge` 只显示是否配置、版本和短 `connection_id`，不会显示长期 token。旧配置没有 `write_backend` 时按 `web` 处理。

## init

```bash
zot --json config init --library-id 123456 --api-key abcd
zot --json config init --target-profile work --library-id 123456 --api-key abcd --make-default
```

说明：

- 不带 `--target-profile` 时，写到根配置
- 带 `--target-profile` 时，写到命名 profile
- `--make-default` 会同步更新默认 profile
- 未显式提供 `data-dir` 时，会尝试自动探测 Zotero 数据目录

## set

```bash
zot --json config set library-id 123456
zot --json config set api-key abcd --target-profile work
zot --json config set write-backend web --target-profile work
zot --json config set embedding-url https://api.example.com/v1/embeddings
```

支持的 key：

- `data-dir`
- `library-id`
- `api-key`
- `semantic-scholar-api-key`
- `embedding-url`
- `embedding-key`
- `embedding-model`
- `output-format`
- `output-limit`
- `export-style`
- `write-backend`（`web` 或 `desktop`）

说明：

- `embedding-*` 只支持根配置，不支持 `--target-profile`
- `output-limit` 需要正整数

## desktop bridge

```bash
zot --json bridge setup
zot --json bridge pair PAIR-CODE
zot --json bridge status
zot --json bridge revoke
```

- `setup` 只生成 XPI 并打开目录；插件需要用户手动安装并重启 Zotero
- 配对码由 Zotero UI 显示，五分钟过期且单次使用；不要把真实 code 或 token 放进日志、issue 或 prompt
- 配对成功会把当前配置目标的 `write_backend` 设为 `desktop`
- 临时覆盖使用 global `--write-backend desktop|web`，不会写回 config
- desktop 未安装、Zotero 未运行、鉴权或协议失败时不会自动 fallback 到 Web
- desktop 第一阶段只支持 merge/dedupe；其他 mutation 仍需要显式 Web backend 和 Web 凭据

## profiles

```bash
zot --json config profiles list
zot --json config profiles use work
```

用途：

- 看有哪些命名 profile
- 把默认 profile 切到某个命名 profile

## 推荐用法

如果只是让 Claude Code / Codex 做 Zotero 任务，优先还是走 skills 页。

只有在这些场景下，才直接看 `config`：

- 环境刚装好，要初始化写凭据
- 需要安装、配对或撤销 desktop bridge
- 默认 profile 不对
- doctor 报配置缺失
- 需要切换 profile 再继续任务
