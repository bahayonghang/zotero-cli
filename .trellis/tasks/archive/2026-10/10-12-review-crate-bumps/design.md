# 设计：四个 crate 锁升级

## 边界

只改 `Cargo.lock`。不改 `Cargo.toml`，因为 clap、thiserror、tar、http 的现有 caret 要求已经包含目标版本。http 没有成员 crate 使用 `http.workspace = true`，所以不新增直接依赖。

更新命令必须带当前版本，避免 Cargo 把传递的 thiserror 1.0.69 也匹配到 `--precise 2.0.20`：

```text
cargo update -p clap@4.6.1 --precise 4.6.6
cargo update -p thiserror@2.0.18 --precise 2.0.20
cargo update -p tar@0.4.45 --precise 0.4.46
cargo update -p http@1.4.0 --precise 1.5.0
```

clap 自己的 `clap_builder` / `clap_derive` 若随 4.6.6 一起变，属于这次升级。记录它们的新版本。`clap_complete` 的要求保持 `4.6.1`；锁版本可以在 4.6 内被解析器带动，但不能跨出 4.6。

## 行为

- clap 4.6.6 只增加 `Command::get_overridden_usage`。
- thiserror 2.0.19 换成 syn 3，2.0.20 调整生成代码的 clippy lint。2026-09-13 的 Rust 1.85 job 已通过这个组合。
- tar 0.4.46 修 PAX header desync。`pdf.rs` 继续用 `Archive`、`Builder`、GNU header 和 symlink entry。
- http 1.5.0 的 PathAndQuery 上限是 65534 字节。现有代码通过 `url::Url` 进入 reqwest，不直接调用 `http::Uri`。

## 回滚

一个提交包含四次精确更新。失败时 revert 该提交，并且不关闭尚未成功升级的 PR。
