# PR triage，2026-11-04

仓库 `bahayonghang/zotero-cli`。默认分支 `main`。`origin/main` 是 `e04a7f8817713eb7ae132d082ba3f401d1390d4a`。`origin/dev` 是 `963585885ec8eaa52ed7bdf54b4908a97d070ad3`。本地 `dev` 是 `e3926bf`，超前 `origin/dev` 3 个提交。

`deleteBranchOnMerge` 为 false。merge、squash、rebase 都允许。5 个 PR 的 `mergeable` 都是 `MERGEABLE`，`mergeStateStatus` 都是 `UNSTABLE`。没有 review decision。

`git ls-remote --heads origin dependabot/*` 只有下面 5 个分支。本克隆 `git branch` 只有 `dev` 和 `main`，没有本地 `dependabot/*` 分支。本地存在对应的 `remotes/origin/dependabot/*` 跟踪引用。

## PR

| PR | Head | 头 | 文件 | 锁里的实际版本 | 非安全类 CI | 安全 job |
| --- | --- | --- | --- | --- | --- | --- |
| #30 | `dependabot/cargo/uuid-1.26.1` | `fd15c9d11f285e7ea9d9a0c521ed8e5b88192bc3` | `Cargo.lock` | uuid 1.23.1 → 1.29.0 | MSRV 失败；stable 三平台、machete、nightly unused、docs 成功 | 失败 |
| #31 | `dependabot/cargo/serde-1.0.229` | `e8f9439d614887abfa06f8ea4afe822014ffe435` | `Cargo.lock` | serde / serde_core 1.0.228 → 1.0.229 | 均成功 | 失败 |
| #32 | `dependabot/cargo/md5-0.8.1` | `de6c6c20ae37fdb30ce0a0b156b5cc79bc42464a` | `Cargo.lock` | md5 0.8.0 → 0.8.1 | 均成功 | 失败 |
| #33 | `dependabot/cargo/sha2-0.11.1` | `c4d66c0a5e530dd68362a724b5074873a61271aa` | `Cargo.lock`、`Cargo.toml` | sha2 0.10.9 → 0.11.1，digest 0.10.7 → 0.11.3 | MSRV、stable 三平台、nightly unused 失败；machete 与 docs 成功 | 失败 |
| #34 | `dependabot/cargo/toml-1.1.8spec-1.1.0` | `720410c1fd865542fa86ab5f7d3415ce50e9cd71` | `Cargo.lock` | toml 1.1.2+spec-1.1.0 → 1.2.0+spec-1.1.0 | 均成功 | 失败 |

创建时间都在 2026-11-04。标题里的 uuid 1.26.1 和 toml 1.1.8 与锁文件不一致，以锁为准。

## 失败原因

#30 run `41802502396`，MSRV job `126481431931`：`uuid@1.29.0 requires rustc 1.89.0`。根 `Cargo.toml` 第 17 行 `rust-version = "1.85"`。`main` 已有的 ignore 是 uuid `>= 1.27.0`，挡不住 Dependabot 以 1.26.1 为名打开、再把锁解到 1.29.0 的更新。

#33 run `41802531344`，ubuntu job `126481526962`：`error[E0277]`，`Array<u8, ...>: LowerHex` 不满足，位置 `src/zot-local/src/pdf.rs:788`。同一写法还有 `src/zot-cli/src/commands/organize.rs:1157` 和 `src/zot-cli/tests/json_error_contract.rs:208`。`Cargo.toml` 只把 `sha2 = "0.10.9"` 改成 `"0.11.1"`。`just version-check` 因此失败。

#31 run `41802512177`，安全 job `126481464919`。`cargo audit` 报 11 个漏洞，crate 是 quinn-proto 0.11.15（RUSTSEC-2026-0338 至 0342）、rustls 0.23.38（RUSTSEC-2026-0285、0346、0348、0349）、rustls-webpki 0.103.13（RUSTSEC-2026-0382）、webpki-root-certs 1.0.6（RUSTSEC-2026-0383）。另有 tokio 1.52.1 的 RUSTSEC-2026-0386 warning。这些都在 PR base `main` 上，不在 #31 的 diff 里。#30、#32、#33、#34 的安全 job 同样失败。

## 两边的锁

`dev` 与 `origin/main` 目前都是：md5 0.8.0、serde 1.0.228、sha2 0.10.9、toml 1.1.2+spec-1.1.0、uuid 1.23.1、digest 0.10.7。

## 合并预演

2026-11-04，`main` 无分支保护，ruleset 数量为 0。按 #31、#32、#34 的顺序对 `origin/main` 做 `git merge-tree --write-tree`，三次退出码都是 0。合成结果相对 `origin/main` 只多出这三份锁差异的并集：

- serde、serde_core、serde_derive：1.0.228 → 1.0.229
- md5：0.8.0 → 0.8.1
- serde_spanned：1.1.1 → 1.1.2
- toml：1.1.2+spec-1.1.0 → 1.2.0+spec-1.1.0
- toml_datetime：1.1.1+spec-1.1.0 → 1.1.2+spec-1.1.0
- toml_parser：1.1.2+spec-1.1.0 → 1.1.5+spec-1.1.0
- toml_writer：1.1.1+spec-1.1.0 → 1.2.0+spec-1.1.0

sha2 与 uuid 不在这个并集里。`.github/dependabot.yml` 与这三份 diff 不相交。
