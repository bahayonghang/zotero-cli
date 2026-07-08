# pdf.rs 第二 HTTP 栈:统一到共享 transport 或记录豁免

## Goal

`pdf.rs` 使用 `reqwest::blocking`(pdf.rs:8)自建 HTTP 栈下载 Pdfium 动态库,与 zot-remote 的 `HttpRuntime` 并存——两套超时/User-Agent/代理行为可能漂移。做一次明确决定:统一、部分对齐、或豁免并记录。这是调查决策型任务,结论可能是「不改代码,只写 spec」。

## Problem & Evidence(2026-07-07 探查)

- pdf.rs 的 Pdfium 下载路径(probe/download,:550、:687 一带)为 blocking HTTP;超时/UA 与 HttpRuntime(http.rs:16-18,15s/60s/zot-cli UA)各自为政
- 跨 crate 约束(评审确认正确、应保持):zot-local 不依赖 zot-remote——不能简单 import HttpRuntime
- 使用场景:一次性引导下载(首次运行/缺库),blocking 在同步 crate 里有其合理性

## Requirements

- 与 07-07-remote-transport 同期评估,产出三选一并记录:
  1. 统一:经 composition root(zot-cli)注入下载能力;
  2. 对齐:抽最小共享常量(超时/UA)到 zot-core,两栈各自使用;
  3. 豁免:保留现状,理由写入 `.trellis/spec/zot-local/backend/`(供未来评审不再重提)
- 无论何种结论,超时与 User-Agent 行为与主栈对齐,或差异被明确记录

## Acceptance Criteria

- [ ] 决定落地并记录(代码或 spec 文档,二者其一)
- [ ] 若统一/对齐:下载路径有冒烟验证;`cargo clippy` / `cargo test` 全绿
- [ ] zot-local 不新增对 zot-remote 的依赖

## Notes

- 轻量调查型:PRD-only。
- 父任务:07-07-arch-deepening(评审小信号 4)。
