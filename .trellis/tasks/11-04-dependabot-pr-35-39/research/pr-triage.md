# PR #35–#39 分诊

记录时间：2026-11-04。仓库 `bahayonghang/zotero-cli`。默认分支 `main`。`origin/main` 为 `f49662e7c4b0f302b83ecef7b1fb3c510ef9b7b2`。`origin/dev` 为 `963585885ec8eaa52ed7bdf54b4908a97d070ad3`。本地 `dev` 为 `da2fbfade2f34fca832dfa9a1c3265d435cc1b2e`，比 `origin/dev` 超前 6 个提交。

`main` 没有分支保护，ruleset 数量为 0。允许 merge、squash、rebase。`deleteBranchOnMerge` 为 false。五个 PR 的 base 都是 `main`，作者都是 Dependabot，`mergeable` 为 `MERGEABLE`，`mergeStateStatus` 为 `UNSTABLE`，没有 review。本克隆没有本地 `dependabot/*` 分支，只有 5 个 `remotes/origin/dependabot/*`。

`origin/main` 的 `Cargo.toml` 仍是 `rust-version = "1.85"`，`clap_complete = "4.6.1"`，`opener = "0.8.4"`，`pdfium-render` `0.9.0`，`rusqlite` `0.39.0`，`thiserror = "2.0.18"`。`origin/main` 的 `.github/dependabot.yml` 已是 cargo / npm / actions `monthly`，并忽略 uuid `>= 1.24.0`、sha2 `>= 0.11.0`、quick-xml `>= 0.42.0`，以及 `dtolnay/rust-toolchain`。本地 `dev` 上的同名文件仍是 cargo / npm weekly，没有这三条 crate ignore。

`origin/main` 最新 CI `41812776799` 里，只有 `dependency security and policy` 失败。stable 三平台、`Rust 1.85.0 / locked workspace`、machete、udeps、docs 都成功。这条安全失败是 `main` 上原有的，不作为这五个 PR 的合并门槛。不改 audit、deny，也不把安全检查改成通过。

## 逐个 PR

| PR | 头 | 文件 | 非安全 CI | 处置 |
| --- | --- | --- | --- | --- |
| #36 thiserror 2.0.18 → 2.0.21 | `68dc9d5f3e211bbcd457c95f3f00b6ad2363997e` | 只改 `Cargo.lock` | 全部成功 | 合并 |
| #37 opener 0.8.4 → 0.9.0 | `a2ad0540fb21621c688922eeb185849eb39b33d4` | `Cargo.toml` + `Cargo.lock` | 全部成功 | 合并 |
| #39 clap_complete 4.6.2 → 4.6.11 | `49acbba05f7b8d058a2887ddd6c194904ee39c2d` | 只改 `Cargo.lock` | 全部成功 | 合并 |
| #38 rusqlite 0.39.0 → 0.41.1 | `71280ddf3d11b26022e7c3f1724dc6eea0e139b3` | `Cargo.toml` + `Cargo.lock` | Rust 1.85 失败；stable 三平台成功 | 关闭 |
| #35 pdfium-render 0.9.0 → 0.9.5 | `4590e9b04d22624afabd9ba02913f4b150634011` | 只改 `Cargo.lock` | stable 三平台、Rust 1.85、nightly udeps 失败 | 关闭 |

#36 把直接依赖 `thiserror` 和 `thiserror-impl` 锁到 2.0.21。`thiserror-impl` 2.0.21 依赖 `syn` 3.0.9。锁里仍保留 `thiserror` 1.0.69 和 `syn` 2.0.117。`Cargo.toml` 的要求仍是 `thiserror = "2.0.18"`。

#37 把 `Cargo.toml` 的 `opener` 要求改成 `0.9.0`。`src/zot-cli/src/util.rs:75` 的 `opener::open` 在 Rust 1.85 和三个 stable 平台都编过。锁同时去掉 `bstr` 1.12.1，并把多处 `windows-sys` 收到 0.61.2。

#39 把 `clap_complete` 锁到 4.6.11，不改 `Cargo.toml`。锁还删掉 `windows-sys` 0.60.2 和 `windows-targets` 0.53.5，有一处依赖从 `windows-sys` 0.60.2 改到 0.52.0。Windows stable 和 Rust 1.85 都成功。

#38 的 Rust 1.85 作业 `126516823576` 报 `rusqlite@0.41.1 requires rustc 1.89.0` 和 `libsqlite3-sys@0.39.1 requires rustc 1.88.0`。锁从 `rusqlite` 0.39.0 / `libsqlite3-sys` 0.37.0 升到 0.41.1 / 0.39.1，`Cargo.toml` 要求也改成 0.41.1。lib.rs 的版本表里，0.39.0 的 MSRV 约 1.82–1.84，0.40.0、0.40.1、0.40.2 约 1.86，0.41.0 和 0.41.1 是 1.89。所以 ignore 用 `>= 0.40.0`，而不是只挡 0.41.1。

#35 只改锁里的 `pdfium-render` 版本和 checksum。Ubuntu 作业 `126516115364` 的错误是 `src/zot-local/src/pdf.rs:804`：`Ok(Pdfium)`，`error[E0423]: cannot find value Pdfium in this scope`。0.9.5 的 `pdfium.rs:88` 是 `pub struct Pdfium {`。现在的调用是 `Pdfium::new(bindings)` 加上已初始化分支的 `Ok(Pdfium)`。0.9.5 的 release note 还把 `pdfium_latest` 指到 Pdfium 8125。本仓库 `src/zot-local/src/pdf.rs:60` 仍固定 `PDFIUM_VERSION = "7543"`，并自带 sha256。这次不改这个调用，也不改下载目标。

## 合并预演

在临时 detached worktree 里，从 `origin/main` 按 #36、#37、#39 顺序 `git merge --no-ff`。三次都干净。合成结果只存在本地预览对象 `bc90caa17fe8669ffaad12b090ed7fb66873dc54`，没有推送。预览锁是 `clap_complete` 4.6.11、`opener` 0.9.0、`thiserror` 2.0.21、`thiserror-impl` 2.0.21、`syn` 3.0.9 与原有的 `syn` 2.0.117。`pdfium-render` 仍是 0.9.0，`rusqlite` 仍是 0.39.0。预览 worktree 已删除。
