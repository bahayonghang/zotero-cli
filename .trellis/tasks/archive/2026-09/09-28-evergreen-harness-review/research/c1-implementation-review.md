# C1 实施与依赖复核

日期：2026-09-28。适用工具：Claude Code、Codex、Grok Build、Kimi Code、OMP。五套工具共享同一锁文件、TLS 边界和验证要求。

## 结果与范围

已将 rustls 精确更新到 0.23.45。`cargo audit` 和 `cargo deny check` 均退出 0。产品文件只修改 `Cargo.lock`；根 `Cargo.toml` 和业务源码未修改。该记录不代表完整验收完成，正式构建门由主代理执行。

工作树基于提交 `ba4e10e987e7afe65dccb8450445d41bc9e08d79`。本次没有创建提交，不能把该基线 SHA 描述为修补后的提交。验证结果绑定以下锁文件内容：

- SHA-256：`857974bce8e060ed25212b68463e2bd1e0c42be3dce908517d001428d93e706f`
- Git blob：`6416182d6ab5b0fb17b202ed044a1c832fc85f0f`

未来提交需记录包含该锁内容的新 commit SHA；对应 hosted CI 尚未运行。

## 公告与版本选择

在线 `cargo audit` 刷新 RustSec 数据库成功，数据库提交为 `e2111519ba6d14a5da59a7b2e5c8083ae8a37c01`。`cargo deny` 使用的数据库提交相同。扫描共 305 个锁文件依赖，加载 1271 条公告。

`RUSTSEC-2026-0285` 将 `>= 0.23.45` 列为修补范围，`< 0.23.13` 列为不受影响范围。当前受影响版本 0.23.38 升级至首个修补版本 0.23.45。公告内容已保存为 `c1-rustsec-advisory.md`。本次没有实施攻击复现。

执行命令为 `cargo update -p rustls --precise 0.23.45 --offline`。使用本地 registry 缓存解析精确补丁；随后安全检查成功在线刷新公告。未执行全量 `cargo update`，未添加 advisory ignore，未关闭 TLS 验证。

## 传递依赖复核

| 包 | 原版本 | 新版本 | 声明的 Rust 版本 | 更新必要性 |
| --- | --- | --- | --- | --- |
| rustls | 0.23.38 | 0.23.45 | 1.71 | 公告首个修补版本 |
| aws-lc-rs | 1.16.2 | 1.18.1 | 1.71.0 | rustls 0.23.45 要求 `^1.18` |
| aws-lc-sys | 0.39.1 | 0.45.0 | 1.71.0 | aws-lc-rs 1.18.1 要求 `^0.45.0` |
| rustls-webpki | 0.103.13 | 0.103.15 | 1.71 | rustls 0.23.45 要求 `^0.103.14` |

解析器为需要更新的传递包选择兼容版本。没有新增或删除包，包数保持 305。`aws-lc-sys` 新增指向已有 `pkg-config` 包的构建依赖；`pkg-config` 版本未变。`reqwest` 仍为 0.13.2；TLS provider 和平台验证器未变。

`aws-lc-sys` 升级跨越其 0.x 次版本区间，因此必须保留 MSRV 与原生构建检查。声明的 MSRV 低于项目 Rust 1.85 仅是元数据证据，不代替实际编译。主代理负责该验证。

`cargo tree -i rustls --locked` 确认实际反向链经过 `hyper-rustls`、`reqwest`、`rustls-platform-verifier` 和 `tokio-rustls`，最终由 zot-desktop、zot-local、zot-remote 使用。完整元数据与锁差异见 `c1-dependency-changes.json`、`c1-lock.diff`。

## 本代理执行的检查

| 命令 | 结果 | 证据 |
| --- | --- | --- |
| `cargo update -p rustls --precise 0.23.45 --offline` | exit 0 | `c1-update.log`、`c1-update-result.json` |
| `cargo tree -i rustls --locked` | exit 0 | `c1-tree.log`、`c1-tree-result.json` |
| `cargo audit` | exit 0 | `c1-cargo-audit.log`、`c1-cargo-audit-result.json` |
| `cargo deny check` | exit 0 | `c1-cargo-deny.log`、`c1-cargo-deny-result.json` |
| `git diff --check -- Cargo.lock Cargo.toml` | exit 0 | `c1-verification-summary.json` |

工具版本：cargo 1.98.0、rustc 1.98.0、cargo-audit 0.22.2、cargo-deny 0.20.2。检查时间、退出码、基线 SHA 与锁文件哈希记录在各结果 JSON。

deny 的 advisories、bans、licenses、sources 均为 ok。仍有 `duplicate` 警告；修补前后的警告类别相同。没有为消除警告增加依赖改造或豁免。

## 环境问题与剩余验证

最初 `cargo info rustls@0.23.45 --locked` 因 crates.io `config.json` 的 schannel TLS 握手失败退出 1。原始错误为 `SSL connect error (schannel: failed to receive handshake, SSL/TLS connection failed)`。同会话 PowerShell `Invoke-WebRequest` 访问该 HTTPS 资源返回 200；原因未查明。后续默认 `cargo audit` 刷新公告和索引成功。未修改 Cargo 网络或 TLS 配置。

按主代理分工，本代理未运行 `just ci`、Rust 1.85.0 locked workspace check、machete 或固定 nightly udeps，避免并发占用构建锁。主代理继续这些必需检查；在全部通过前不能将 C1-AC2 标为完成。

主代理另要求只读检查固定工具的离线缓存。当前默认 `C:/Users/lyh/.cargo` 中没有 cargo-machete 0.9.2 或 cargo-udeps 0.1.61 的二进制、源码目录、`.crate` 文件或对应版本索引缓存；PATH 未找到二进制。当前缓存不足以离线安装。本代理未下载、安装或编译这两个工具。

未提交、推送、归档、修改任务状态或启动其他 harness 会话。跨平台 hosted 结果和修补后 commit SHA 待后续授权流程验证。
