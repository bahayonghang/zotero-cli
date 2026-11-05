# 工作流失败根因

日期2026-09-28。只读 gh 记录，无远端操作。

## 当前安全失败
Cargo.lock:1558-1570 为 rustls0.23.38。audit/deny 均 exit1，RUSTSEC-2026-0285 于2026-09-14发布，CVSS5.3，中等，修补>=0.23.45。依赖链来自 reqwest0.13.2，经 rustls/hyper-rustls/tokio-rustls 被 desktop/local/remote 使用。证明受影响版本存在，未做实际攻击验证。

[同 SHA CI 34434843527](https://github.com/bahayonghang/zotero-cli/actions/runs/34434843527) 于2026-09-10运行，8个job全绿，含三平台stable、MSRV、安全、machete、udeps、docs。早于公告4天；旧全绿不能证明当前安全。证据 hosted-baseline.json。

justfile:78-80 不包含 audit/deny；它们位于 ci.yml:64-89 的独立security job。ci.yml:3-8 无schedule。直接根因是受影响锁文件；长期检测空档来自触发范围与证据过期。分别由C1、C4负责。

## 历史记录
| 运行 | 根因 | 当前结论 |
| --- | --- | --- |
| [30210314607](https://github.com/bahayonghang/zotero-cli/actions/runs/30210314607)，2026-07-26 | clean checkout缺 .agents/skills/zot；nightly日期误作action ref | checker与workflow已修；当前门通过，不重复改 |
| [29644116256](https://github.com/bahayonghang/zotero-cli/actions/runs/29644116256)，2026-07-18 | tag1.0.0被github-pages的v*策略拒绝，build成功 | 已重读annotations和policy；release.md已有正确约定，不放宽策略 |
| [29644167450](https://github.com/bahayonghang/zotero-cli/actions/runs/29644167450) | 同SHA/main手动部署成功 | 不等于未来release路径复验 |
| [28945565328](https://github.com/bahayonghang/zotero-cli/actions/runs/28945565328)，2026-07-08 | cargo fmt报collection.rs、item/write.rs、library.rs、output.rs、graph.rs格式差异 | 当前fmt通过，无新改造 |
| [25625424668](https://github.com/bahayonghang/zotero-cli/actions/runs/25625424668)，旧pdfium PR | 日志HTTP410，原因未查明 | 不据此修改当前PDF依赖 |

证据：historical-ci-failure.log、historical-pages.json、pages-failure-annotations.json、pages-policy.json、pages-runs.json、historical-july-ci.log、historical-pdfium-failure.log。新方案的hosted/schedule/release验收需新SHA和独立授权，不从本地成功推断。
