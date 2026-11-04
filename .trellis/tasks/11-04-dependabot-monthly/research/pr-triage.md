# PR triage，2026-11-04

## 同日复核

本地 `origin/main` 原先停在 `5b368d0`，fetch 后为 `0e6d584`（Merge pull request #16）。`origin/dev` 仍是 `9635858`。`dev` 比 `main` 多 30 个提交，`main` 有 4 个不在 `dev` 的合并提交。`Cargo.lock` 在 `5b368d0..origin/main` 无变化。

5 个 PR 的 base 都是 `0e6d584`，`mergeable` 为 `MERGEABLE`，`mergeStateStatus` 为 `UNSTABLE`。`main` 分支保护返回 404，ruleset 列表为空。`deleteBranchOnMerge` 为 false。merge、squash、rebase 都允许。

`git merge-tree --write-tree` 按 chrono、flate2、clap 顺序合并，结果干净。

`main` 的 `dependabot.yml` 没有 `dtolnay/rust-toolchain` ignore。`dev` 有。Dependabot 读取默认分支 `main`。

仓库 `bahayonghang/zotero-cli`。下面的 PR 表仍使用首次读取的 head。分支关系以本节上面的复核为准：`origin/main` 是 `0e6d584`，`origin/dev` 是 `9635858`。

远程 `git ls-remote --heads origin dependabot/*` 只有下面 5 个分支。截图里的分支列表与此一致，PR 计数为 5。

## PR

| PR | Head | 文件 | 目标版本 | 非安全类 CI | 安全 job |
| --- | --- | --- | --- | --- | --- |
| #25 | `dependabot/cargo/chrono-0.4.45` `fd17fe1` | `Cargo.lock` | chrono 0.4.44 → 0.4.45 | stable 三平台、MSRV、machete、nightly unused、docs 均成功 | 失败 |
| #26 | `dependabot/cargo/flate2-1.1.10` `9ba3047` | `Cargo.lock` | flate2 1.1.9 → 1.1.10 | 同上，均成功 | 失败 |
| #27 | `dependabot/cargo/clap-4.6.7` `8ce3efc` | `Cargo.lock` | 相对 main：clap 4.6.1 → 4.6.7 | 同上，均成功 | 失败 |
| #28 | `dependabot/cargo/uuid-1.28.0` `c320dfc` | `Cargo.lock` | uuid 1.23.1 → 1.28.0 | MSRV 失败；其余非安全 job 成功 | 失败 |
| #29 | `dependabot/cargo/quick-xml-0.42.0` `1cd8c86` | `Cargo.lock`、`Cargo.toml` | quick-xml 0.41.0 → 0.42.0 | stable 三平台、MSRV、nightly unused 失败；machete 与 docs 成功 | 失败 |

创建时间都是 2026-10-18。作者 `app/dependabot`。`mergeable` 均为 `MERGEABLE`。没有 review decision。

## 失败原因

#25 run `39153096652`，安全 job `117750608153`。`cargo audit` 报 9 个漏洞，crate 是 quinn-proto 0.11.15（RUSTSEC-2026-0338 至 0342）和 rustls 0.23.38（RUSTSEC-2026-0285、0346、0348、0349）。这是 PR base `main` 的锁，不是 chrono 引入的。

#28 run `39153114875`，MSRV job `117750666338`：`uuid@1.28.0 requires rustc 1.89.0`。

#29 run `39153121503`，MSRV job：`quick-xml@0.42.0 requires rustc 1.86`。stable ubuntu 编译错误：

- `no method named decode`：`BytesText`、`BytesCData`、`BytesRef`
- `no method named to_vec` for `&str`
- `can't compare str with [u8; 6]` 与 `[u8; 5]`

`Cargo.toml` patch 只有一行：`quick-xml = "0.41.0"` 改为 `"0.42.0"`。

## 本地锁与要求

`dev` 的 `Cargo.lock`：chrono 0.4.44、clap 4.6.6、flate2 1.1.9、uuid 1.23.1、quick-xml 0.41.0、rustls 0.23.45、quinn-proto 0.11.15。

`main` 的 `Cargo.lock`：clap 4.6.1、rustls 0.23.38，其余四个目标包与 `dev` 相同。

workspace 要求：chrono `0.4.42`、clap `4.6.1`、flate2 `1.1.5`、uuid `1.23.1`、quick-xml `0.41.0`。`rust-version = "1.85"`。

## crates.io rust-version

| crate | version | rust-version |
| --- | --- | --- |
| chrono | 0.4.45 | 1.62.0 |
| flate2 | 1.1.10 | 1.67.0 |
| clap | 4.6.7 | 1.85 |
| uuid | 1.23.1–1.26.0 | 1.85.0 |
| uuid | 1.27.0、1.28.0 | 1.89.0 |
| quick-xml | 0.41.0 | 1.79 |
| quick-xml | 0.42.0 | 1.86 |

uuid 1.24.1、1.25.0、1.26.0 存在且 rust-version 为 1.85.0。1.27.1 与 1.28.1 在查询时不存在。

## 当前 dev 的 cargo audit

2026-11-04，`cargo audit` 0.22.2，1403 条 advisory，306 个 crate。退出码 1。11 个漏洞：

| ID | crate | 当前 | 修复版本 |
| --- | --- | --- | --- |
| RUSTSEC-2026-0338 | quinn-proto | 0.11.15 | >=0.11.18 |
| RUSTSEC-2026-0339 | quinn-proto | 0.11.15 | >=0.11.17 |
| RUSTSEC-2026-0340 | quinn-proto | 0.11.15 | >=0.11.18 |
| RUSTSEC-2026-0341 | quinn-proto | 0.11.15 | >=0.11.17 |
| RUSTSEC-2026-0342 | quinn-proto | 0.11.15 | >=0.11.17 |
| RUSTSEC-2026-0346 | rustls | 0.23.45 | >=0.23.46 |
| RUSTSEC-2026-0347 | rustls | 0.23.45 | >=0.23.46 |
| RUSTSEC-2026-0348 | rustls | 0.23.45 | >=0.23.46 |
| RUSTSEC-2026-0349 | rustls | 0.23.45 | >=0.23.46 |
| RUSTSEC-2026-0382 | rustls-webpki | 0.103.15 | >=0.103.16 或指定 alpha |
| RUSTSEC-2026-0383 | webpki-root-certs | 1.0.6 | >=1.0.10 |

另有 RUSTSEC-2026-0386，tokio 1.52.1，warning。这些都不在 #25–#29 的 diff 中。

## Dependabot 配置

`.github/dependabot.yml`：

- cargo `/`：weekly，limit 5，labels `dependencies`、`rust`
- npm `/docs`：weekly，limit 5，labels `dependencies`、`docs`
- github-actions `/`：monthly，ignore `dtolnay/rust-toolchain`，labels `dependencies`、`ci`

GitHub 文档：`monthly` 在每月 1 日运行。`day` 只指定 weekly 的星期几。默认时刻由 Dependabot 随机分配，可用 `time` 与 `timezone` 固定。

## 调用点

quick-xml 解析在 `src/zot-remote/src/oa.rs`。`decode()` 位于约 330、338、348 行。标签名与字节串比较位于约 312–318 行。
