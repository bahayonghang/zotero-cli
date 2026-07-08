# config.rs root/profile 四函数收敛为 apply_setting

## Goal

`apply_root_init` / `apply_profile_init`(config.rs:174-221)与 `apply_root_setting` / `apply_profile_setting`(:223-269)四个函数近乎重复——新增一个配置键最多要改 4 处,是典型的 forgot-to-update-X 温床。收敛为单一 apply 路径 + 一条 profile-only 拒绝规则。

## Problem & Evidence(2026-07-07 探查)

- 四函数结构近同,差异仅在目标(root vs profile)与个别键的允许性
- config.rs 已有 2 个纯 helper 测试可作回归锚
- 本文件另有 5 处 `if ctx.json` 分支,由 07-07-cmd-output 统一处理,本任务不碰输出

## Requirements

- 单一 `apply_setting` 入口(root/profile 目标作参数),init 与 set 两条命令复用
- 现有全部配置键行为不变;profile 不允许的键仍被拒绝且提示文案一致

## Acceptance Criteria

- [ ] 「新增键只需改 1 处」成立(以测试或迁移说明证明)
- [ ] 既有 config 测试全绿,并补 root/profile 两路径的对照测试
- [ ] `cargo clippy` / `cargo test` 全绿

## Notes

- 轻量:PRD-only。
- 父任务:07-07-arch-deepening(评审小信号 3)。
