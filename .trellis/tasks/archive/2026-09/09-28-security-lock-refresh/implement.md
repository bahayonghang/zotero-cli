# C1 Implementation（2026-09-28 已批准）

1. 核对批准、HEAD、Cargo.lock和RustSec是否有新信息；读质量规范。
2. 强模型确认精确补丁与MSRV，低成本执行者可运行 cargo update -p rustls --precise 0.23.45；任何冲突交回，禁止全量update。
3. 读回 cargo tree -i rustls --locked 与锁文件diff；如需manifest改变记录必要性。
4. 必须通过：cargo audit；cargo deny check；just ci；cargo +1.85.0 check --workspace --locked；cargo machete；cargo +nightly-2026-07-01 udeps --workspace --all-targets --locked；git diff --check。
5. 如缺工具，批准范围内准备CI固定版本：Rust1.85.0、nightly-2026-07-01、cargo-machete0.9.2、cargo-udeps0.1.61；不切换全局默认。工具未准备或检查失败时不得宣布完成。
6. 强模型复审仅所需依赖diff；记录日期、命令、版本、退出码和SHA。原生跨平台hosted结果在对应新SHA运行前未验证，不沿用9月10日记录。

Write files: Cargo.lock；条件性 Cargo.toml。证据写父任务research。无远端写。
