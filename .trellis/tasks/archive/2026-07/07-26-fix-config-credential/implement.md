# Implement: 配置与凭据加固

## Ordered Checklist

1. 在 `zot-core` 增加 redacted `SecretString`，替换三个配置层的 key 字段并补 serde/Debug canary 测试。
2. 把 config path 改为 fail-closed result，并将非 secret sidecar 调用迁移到明确的 `state_dir`。
3. 使用同目录临时文件实现原子 save，补覆盖、残留和 Unix mode 测试。
4. 修复 Unicode redaction，补多字节边界回归测试。
5. 增加一次性 effective config/profile 加载，并让 `AppContext` 与 main 错误渲染状态使用 effective profile/JSON。
6. 为白名单只读命令接入配置 limit，保持显式参数优先及写/索引命令默认值。
7. 收紧 `output-format/output-limit` 校验，更新 `config init` root-only 预检和测试。
8. 更新 doctor Web 写能力 schema、测试与双语 config/doctor 文档。
9. 更新适用的 Trellis 后端质量规范，记录 secret Debug、原子配置写入和运行时默认值契约。
10. 完成 PRD AC 勾选、最终 diff 审查和原子提交。

## Focused Validation

```powershell
cargo test -p zot-core config
cargo test -p zot-cli config
cargo test -p zot-cli doctor
cargo test -p zot-cli cli
cargo test -p zot-cli --test json_cli
```

## Full Gate

```powershell
just ci
```

## Risk And Rollback Points

- `SecretString` 会触及 config consumers；每轮先 `cargo check -p zot-cli`，避免批量修改后才发现类型漂移。
- config path 返回类型可能传播到 sidecar；只把 secret 配置 fail-closed，非 secret state 使用独立 API。
- limit 只走显式白名单，禁止用字段名批量替换所有 `limit`。
- main 错误渲染变更必须复验 JSON parse/runtime 两类错误，不能只测成功 envelope。
- 不修改后续 P2 子任务目录或父任务源报告。
