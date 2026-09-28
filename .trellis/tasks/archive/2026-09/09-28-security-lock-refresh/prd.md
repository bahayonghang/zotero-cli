# C1 / P1：修复 rustls 安全依赖失败

## Goal
消除已确认的安全门失败，保持 Rust 1.85 与 HTTP 行为兼容。

## Evidence
父任务 research/cargo-audit.log、cargo-deny.log 均 exit1。Cargo.lock:1558-1570 锁定 rustls0.23.38；RUSTSEC-2026-0285 于2026-09-14公布，修补>=0.23.45。根依赖 reqwest0.13.2 在 Cargo.toml:39-45 启用 rustls，由 desktop/local/remote 使用。

## Requirements
- C1-R1：优先只对 rustls 执行精确补丁更新，不全量 cargo update。
- C1-R2：不 ignore advisory、不关闭TLS验证、不改后端；保留MSRV和锁文件约束。
- C1-R3：锁文件与验证结果绑定新SHA；记录变动的传递依赖。

## Acceptance Criteria
- C1-AC1 / R1：锁文件不含公告影响范围内 rustls，cargo audit 与 cargo deny check 均 exit0。
- C1-AC2 / R2：just ci、Rust1.85 locked check、machete、固定nightly udeps全部通过。
- C1-AC3 / R3：diff仅为所需依赖锁变动及必要最小manifest约束；没有ignore、源码绕过或无关版本升级；强模型审核依赖树。

## Scope
写：Cargo.lock；仅当最小补丁解析确实需要时写根Cargo.toml。依赖约束需要大版本升级或MSRV变化则停下提交修订方案。只读：三个使用reqwest的crate manifests、deny.toml、ci.yml、RustSec与质量规范。不改业务源码。

## Authority and Dependency
2026-09-28 已批准，无上游任务依赖。允许准备项目CI固定的验证工具，不改全局默认工具链。push/远端CI/发布另需授权。适用五套harness；强模型决定补丁与兼容性，低成本模型可执行精确命令。
