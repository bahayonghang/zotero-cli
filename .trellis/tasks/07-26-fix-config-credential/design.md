# Design: 配置与凭据加固

## Boundaries

- `zot-core::config` 负责 secret 类型、配置路径、materialization、redaction 和持久化事务。
- `zot-cli::context` 负责把 effective profile、effective JSON mode 和已 materialize 配置装入一次请求上下文。
- `zot-cli::cli` 负责按命令白名单补全只读 `--limit`，不让配置穿透到写入安全边界。
- `doctor` 与 `config` command 只消费上述契约，不自行推断 secret 或 profile。

## Secret Representation

新增 serde-transparent `SecretString(String)`，只暴露 `is_empty`、`expose_secret` 和受控赋值。其 `Debug` 固定显示 redacted 状态，不实现 `Display` 或 `Deref<Target=str>`，避免格式化和隐式转换泄漏。包含 secret 的配置结构可以继续派生 `Debug`，安全性由字段类型封装。

## Config Path And Save Transaction

`AppConfig::config_dir/config_file` 改为可失败结果，底层纯函数接受 `Option<PathBuf>` 以覆盖无系统目录测试。已有 cache/index/workspace 路径迁移到明确命名的 `state_dir`；state fallback 与 secret config path 分离。

保存顺序：

1. 解析受信系统 config dir 并创建目录。
2. 在同目录创建 `NamedTempFile`。
3. Unix 在写入前显式设为 `0600`。
4. `write_all`、`flush`、`sync_all`。
5. `persist` 原子替换目标；Unix 再 `sync_all` 父目录。

临时文件对象负责失败清理，目标文件直到第 5 步才改变。

## Effective Runtime Options

`AppConfig::load_effective(explicit_profile)` 一次返回 `(materialized_config, effective_profile)`。`AppContext::from_cli` 从配置计算：

- `profile = explicit.or(default)`；
- `json = cli.json || config.output.default_format == "json"`。

主流程在 context 构造后更新错误渲染状态，再校验 JSON 协议并补全命令默认值。这样 dispatch 失败使用 effective JSON/profile；CLI parse 或 config load 失败只能使用显式 flags。

命令参数的可配置 limit 改为 `Option<usize>`；`Cli::resolve_effective_options(limit)` 仅填充 PRD 白名单。非白名单参数保留现有 concrete defaults。该方法在 dispatch 前只调用一次，handlers 继续收到 concrete `usize`（通过小型解析后参数类型或白名单 helper），避免在每个 handler 复制配置解析。

## Compatibility

- TOML secret 字段仍是普通字符串，现有配置无需迁移。
- 默认 `output.default_format=table`；显式 `--json` 行为不变。
- 显式 `--limit` 总是胜过配置。
- doctor Web capability 是 JSON 字段收紧：删除含混 `available`，新增诚实的验证状态字段；human output 已使用 `configured` 文案。

## Failure And Rollback

- 配置目录不可用：返回 `config-dir-unavailable`，不尝试 CWD。
- 非法 output format/limit：load 或 set 阶段返回 `config-value`，不 dispatch。
- profile init root-only 参数：在 mutation 前完成预检，保证全有或全无。
- 原子替换失败：保留旧目标，临时文件由 RAII 清理。

## Test Strategy

- `zot-core`: secret serde/debug canary、Unicode redaction、路径 fail-closed helper、原子覆盖与 Unix mode。
- `zot-cli`: context effective profile/JSON、limit 白名单/显式优先、JSON protocol、doctor schema、config init 原子 fail-fast。
- integration: 临时 config 下运行 CLI，断言默认 profile、默认 JSON 和 runtime error envelope。
