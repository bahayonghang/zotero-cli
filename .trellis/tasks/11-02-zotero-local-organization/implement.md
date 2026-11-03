# Implementation plan — 父任务收口

## Before implementation

- [x] 子任务 artifacts 记录已有规划批准；用户交接后明确要求继续检查和实施。
- [x] 确认工作区状态；读取对应子任务 artifacts 与 trellis-before-dev，按 codex inline 执行。
- [x] A/B/C 均已 in_progress；父任务不替代实际交付任务。
- [x] 本轮重新运行 doctor，固定 cargo run -q -p zot-cli -- --json；环境变化见 validation.md。
- [x] 未创建 GitHub issue、提交、推送或修改用户主库；隔离桌面操作已单独授权并完成；原实例已恢复。

## Delivery order

1. A：本地单条标签，完成其自动化、文档和隔离真实验收。
2. B：本地分类创建，先完成其库版本/POST 协议核查。
3. C：冻结计划与批量执行；复用已验收的 A/成员算法，最后串联 B。
4. 父任务：对照 R1-R7 与 AC1-AC8 核对完整闭环及证据。

## Quality gate

每个子任务使用与修改匹配的 focused tests；代码稳定后：

- cargo fmt --all --check
- just ci
- npm --prefix docs run build
- git diff --check

有意修改 canonical skills 时才运行 just skills-sync，然后检查生成镜像；禁止用 ci 自动修写文件。inline 模式不要求创建无意义 JSONL 派发清单。

## End-to-end acceptance

- [x] 隔离 profile 和 data，确认不同 Server ID，无 Web 凭据、不登录同步。
- [x] 创建根/子分类 → 三条以上合成文献的不同标签/成员计划 → 检查差异 → 用户授权 → 执行 → API/UI/重启回读。
- [x] 同时验证原有无关标签类型、其他分类、children 和附件引用不变。
- [x] 故障场景区分真实和 mock：过期版本、拒绝授权、只读组库、提交中断、部分成功。
- [x] 真实组库/其他 OS 未实测时明确限制，不用 mock 冒充桌面证据。
- [x] 将脱敏 evidence 与 AC 映射写入 validation.md，恢复原桌面实例。
- [ ] 子任务完成后依次归档；所有 AC 达成后才收口父任务。

## Rollback / stop

按子任务边界停止，不越界清理用户改动。产品代码回退不自动回滚 Zotero 数据；测试数据只限隔离库。版本/返回协议不符合设计时先修订规划，不降级为无条件写入。
