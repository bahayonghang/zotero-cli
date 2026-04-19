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
- 排查写权限、embedding、data dir 是否缺失

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

说明：

- `embedding-*` 只支持根配置，不支持 `--target-profile`
- `output-limit` 需要正整数

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
- 默认 profile 不对
- doctor 报配置缺失
- 需要切换 profile 再继续任务
