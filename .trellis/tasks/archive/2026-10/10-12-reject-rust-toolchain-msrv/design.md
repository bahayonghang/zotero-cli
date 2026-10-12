# 设计：阻止 toolchain 引用被 Dependabot 升级

## 边界

只改 `.github/dependabot.yml` 的 `package-ecosystem: github-actions` 条目。在 `schedule` 同级增加：

```yaml
ignore:
  - dependency-name: "dtolnay/rust-toolchain"
```

不写 `update-types`，因为 `@1.85.0` 到 `@1.120.0` 会被 Dependabot 看成版本升级，而 `@stable` 也不是普通 semver。整段依赖名忽略才能覆盖这三种引用。

cargo 与 npm 两个 ecosystem 不加 ignore。

## 为什么不改 ci.yml

MSRV job 的 `@1.85.0` 已经是正确钉扎。#24 的 diff 不进入 `dev`。Actions 子任务只替换 checkout 和 setup-node，不应碰到这一行；本任务再核对一次。

## 回滚

revert `dependabot.yml` 提交会恢复 Dependabot 打开同类 PR 的能力。#24 保持关闭。
