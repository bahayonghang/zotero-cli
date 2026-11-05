# dirs 6.0.0 与 7.0.0 的路径差异

来源：2026-10-12 从 crates.io 下载 `dirs-6.0.0.crate` 与 `dirs-7.0.0.crate` 后比较 `src/`。未把第三方源码留在仓库里。

两个 crate 的 `Cargo.toml` 都依赖 `dirs-sys` 0.5.0。crates.io 上 `dirs-sys` 的 0.5 线只有 0.5.0（2025-01-11），没有更新的补丁版。

`src/` 哈希：

| 文件 | 结果 |
| --- | --- |
| `lib.rs` | 相同 |
| `lin.rs` | 相同 |
| `mac.rs` | 相同 |
| `wasm.rs` | 相同 |
| `win.rs` | 不同 |

`win.rs` 只有一行不同：`preference_dir` 从 `known_folder_local_app_data` 改为 `known_folder_roaming_app_data`。`config_dir`、`cache_dir`、`home_dir` 不在差异里。Linux 与 macOS 源码相同。

本仓库没有 `preference_dir` 调用。配置与状态用 `config_dir`，PDF 缓存用 `cache_dir`，`home_dir` 在 `src/zot-core/src/config.rs`。因此 7.0.0 不改变现有路径。
