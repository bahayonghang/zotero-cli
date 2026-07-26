# P1:Pdfium下载校验+原子安装

## Goal

把 Pdfium 首次自动下载从“HTTPS 后直接解压到可加载路径”改为有固定 trust anchor、
资源上限、跨进程串行化和原子发布的 verified installer；任何下载、校验或解压失败
都不得产生新的可加载 native library，也不得覆盖已验证版本。

## Background

- 审计证据 `zotero-cli-code-audit-2026-07-25.md:118,763-770` 确认当前
  `src/zot-local/src/pdf.rs:708-830` 将 `.tgz` 全量读入内存，无 checksum/size cap，
  并直接 `unpack()` 到最终候选路径。
- 已归档 P0 `07-26-fix-pdfium-cwd-rce` 已将加载候选收敛为显式 env、exe-adjacent
  与 managed cache；本任务依赖并保持该顺序，不重新实施 P0，也不扩大 trust roots。
- GitHub release API 对 Chromium 7543 资产提供 SHA-256 digest；本任务研究已下载当前
  7 个支持资产交叉核验归档 digest，并固定解压后动态库 digest。

## Requirements

### R1 固定版本与平台 manifest

- `PdfiumDownloadTarget` 必须同时包含 archive name、archive 内动态库路径、官方
  archive SHA-256 与已核验 library SHA-256；7 个现有支持目标必须完整映射。
- managed cache 只返回与当前版本/平台 manifest 匹配且 library SHA-256 正确的路径。
  旧版裸缓存文件或被改写的 verified 文件不得成为加载候选。
- 平台/资产更新必须显式更新版本、两个 digest 与测试映射，不能从网络动态接受新 hash。

### R2 有界流式下载与 fail-closed 校验

- 下载写入 cache 目录内的临时文件，同时流式计算 SHA-256；不得把完整 archive
  读入 `Vec<u8>`。
- `Content-Length` 和实际读取字节数都必须受 32 MiB 上限约束；超限、截断、网络读
  失败或 checksum mismatch 返回稳定 typed error，临时文件自动清理。
- archive checksum 通过后才允许打开 tar；只复制 manifest 指定的 regular-file entry，
  解压后的动态库受 128 MiB 上限约束并再次校验 SHA-256。

### R3 锁、同步与原子发布

- cache 目录内使用跨平台文件 advisory lock 串行化首次安装；锁随 file handle/process
  退出自动释放，不使用可能永久残留的 create-new lock directory。
- archive temp 与 library temp 在进入下一阶段前 flush/sync；library temp 必须与最终
  文件同目录并通过 atomic persist/rename 发布。
- 获取锁后必须重新检查已验证最终文件，确保并发调用只下载/安装一次。
- 只有新 archive 与 library 全部校验成功后才可移除同名无效目标并发布；失败不得
  覆盖或删除任何已验证 library，旧裸缓存保持未加载但不做无关清理。

### R4 依赖与范围

- 使用 `sha2 0.10.9` 和 `fs4 1.1.0`；后者 MSRV 1.75，低于 workspace Rust 1.85。
- 不改变显式 env/executable-adjacent 候选语义，不新增网络来源，不实现在线
  GitHub/Sigstore attestation 或父任务范围外 L-03 release provenance。

## Acceptance Criteria

- [x] manifest 测试精确覆盖 7 个平台资产及 archive/library digest。
- [x] tampered、truncated、wrong-platform archive 和超限输入均 fail closed，最终候选
      不存在或原有 verified library 内容不变。
- [x] archive 只接受预期 regular-file entry；missing/wrong entry 与 library hash
      mismatch 不发布文件。
- [x] 并发 installer 测试证明多个调用共享一个最终路径且下载 closure 只执行一次。
- [x] managed candidate 测试证明旧裸缓存和 hash 不匹配文件不会被加载，正确 verified
      文件仍保持 P0 的 trusted-source 顺序。
- [x] `cargo test -p zot-local`、相关 doctor/CLI 测试和最终 `just ci` 全部通过。

## Out Of Scope

- 新增平台、改变 `PDFIUM_VERSION`、发布签名/provenance/SBOM。
- 通用 native artifact installer 框架或 zot-remote HTTP 栈合并。
- 删除用户旧缓存、修改显式 operator-provided Pdfium 路径的信任语义。
