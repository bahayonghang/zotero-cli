# 设计：dirs 7

## 边界

改根 `Cargo.toml` 的一行和 `Cargo.lock`。`src/zot-core/src/config.rs` 与 `src/zot-local/src/pdf.rs` 的调用保持原样。

`dirs = "6.0.0"` 改为 `dirs = "7.0.0"`，与仓库里其他三位版本的 caret 要求一致。然后：

```text
cargo update -p dirs@6.0.0 --precise 7.0.0
```

`dirs-sys` 必须仍是 0.5.0。crates.io 上没有更新的 0.5 补丁，正常解析不应离开这个版本。

## 路径

Linux 与 macOS 实现文件相同。Windows 只改变未使用的 `preference_dir`。`config_dir` 继续是 RoamingAppData，`cache_dir` 继续是 LocalAppData，因此 `AGENTS.md` 里的 `~/.config/zot`、`~/Library/Application Support/zot` 和 `%AppData%\zot` 仍然成立。

## 回滚

revert 该提交会同时恢复要求字符串和锁文件。
