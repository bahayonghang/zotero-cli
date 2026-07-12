# Implement: 第一阶段路线图

## Activation Order

父任务没有直接实现。用户审批后按顺序启动并完成子任务：

1. `07-11-zotero-bridge-foundation`
2. `07-11-local-merge-dedupe`
3. `07-11-local-write-skill-docs`

每个子任务开始前运行 `trellis-before-dev`，读取自己的 prd/design/implement、
`get_context.py --mode packages`、相关 spec index/checklist 和 shared guides。不要对父任务
运行 `task.py start`。

## Integration Checklist

- [ ] foundation 提供可安装 XPI、协议 v1、Rust client、config/backend selection、
  setup/pair/status/revoke 和 doctor 四能力状态。
- [ ] merge 子任务只复用 foundation 公开 client/DTO，不绕过鉴权或直接调插件内部。
- [ ] skill/docs 子任务只记录已通过测试的 CLI surface 和 envelope。
- [ ] 三个子任务依赖和验收在各自工件中显式存在。
- [ ] 旧 profile migration、explicit web、desktop error no-fallback 做跨任务回归。
- [ ] canonical skill 安装镜像、docs、XPI manifest 和 workspace version 一致。

## Final Validation

```powershell
just ci
just xpi-check
npm --prefix docs run build
just install
```

另需运行：

- foundation fake-server/protocol tests；
- skill eval 与修改前 snapshot baseline；
- 隔离 Zotero 9 profile 的 install/pair/revoke/uninstall smoke；
- fixture collection 的 dedupe dry-run、normal-only apply、low-confidence skip、恢复演练；
- `git diff --check` 和 secret pattern scan。

## Review Gate

进入实施前检查：

- [ ] `prd.md` 无已解决 open question 或重复事实。
- [ ] 父/子任务均有 `prd.md`、`design.md`、`implement.md`。
- [ ] inline workflow 不要求 curated `implement.jsonl` / `check.jsonl`。
- [ ] 用户明确批准本规划。
- [ ] 批准后只启动 foundation child，并在写代码前执行 `trellis-before-dev`。
