# 执行：dirs 7

工作分支：本地 `dev`。开始前，`10-12-review-crate-bumps` 的锁文件提交必须已经在 `dev` 上。

## 清单

- [ ] 把 `Cargo.toml` 的 `dirs` 要求改为 `7.0.0`。
- [ ] 运行 `cargo update -p dirs@6.0.0 --precise 7.0.0`。
- [ ] 确认锁里 dirs 是 7.0.0、dirs-sys 是 0.5.0、rustls 是 0.23.45。
- [ ] 确认 diff 没有改 `src/` 或 `AGENTS.md`。
- [ ] `cargo test -p zot-core`
- [ ] `cargo test -p zot-local`
- [ ] 提交 `Cargo.toml` 与 `Cargo.lock`。
- [ ] 评论并关闭 #19。评论说明唯一的 Windows 差异是未使用的 `preference_dir`，并写明本地提交哈希尚未推送。

## 回滚点

`git revert` 该提交。不重开 #19，除非用户要求。
