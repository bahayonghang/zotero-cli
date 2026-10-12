# 2026-10-12 锁文件精确更新

命令：

```text
cargo update -p clap@4.6.1 --precise 4.6.6
cargo update -p thiserror@2.0.18 --precise 2.0.20
cargo update -p tar@0.4.45 --precise 0.4.46
cargo update -p http@1.4.0 --precise 1.5.0
```

结果：

| package | before | after |
| --- | --- | --- |
| clap | 4.6.1 | 4.6.6 |
| clap_builder | 4.6.0 | 4.6.6 |
| clap_derive | 4.6.1 | 4.6.4 |
| clap_complete | 4.6.2 | 4.6.2 |
| thiserror | 2.0.18 | 2.0.20 |
| thiserror-impl | 2.0.18 | 2.0.20 |
| thiserror | 1.0.69 | 1.0.69 |
| thiserror-impl | 1.0.69 | 1.0.69 |
| syn | 2.0.117 | 2.0.117，另新增 3.0.8 |
| tar | 0.4.45 | 0.4.46 |
| http | 1.4.0 | 1.5.0 |
| rustls | 0.23.45 | 0.23.45 |
| dirs | 6.0.0 | 6.0.0 |

`Cargo.toml` 未改。`git diff --stat -- Cargo.lock` 为 56 insertions, 45 deletions。
