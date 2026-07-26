# P0热修:移除Pdfium CWD加载路径

## Goal

关闭审计报告 P0-01 / QW-01：`candidate_library_paths()` 将当前工作目录中的同名 Pdfium 动态库纳入加载候选，导致在不可信目录执行 `zot doctor`（以及任何触发 Pdfium probe/load 的命令）时可能加载恶意 `pdfium.dll` / `libpdfium.so` / `libpdfium.dylib` 并执行任意 native code。

本任务是 security hotfix，范围严格收敛为 **删除隐式 CWD 候选 + 删除裸库名 system fallback + 锁定 trust 候选顺序 + 安全回归测试**。下载校验、原子安装、executable-adjacent 所有者检查等完整 hardening 归属后续子任务（`fix-pdfium-download-verify` 等），不在本任务实现。

## Background / Evidence

- 审计条目：`zotero-cli-code-audit-2026-07-25.md` §1.1 P0-01、§6 QW-01
- 核实状态：CONFIRMED（dev@39aaeb4）
- 审计基线代码（dev@39aaeb4）：
  - `src/zot-local/src/pdf.rs` — `candidate_library_paths()` 含 `env::current_dir().join(library_name)`
  - `src/zot-local/src/pdf.rs` — 候选失败后调用 `Pdfium::bind_to_system_library()`；`pdfium-render 0.9.0` 将其实现为 `libloading::Library::new()` 加载平台裸库名，仍会进入平台默认动态库搜索路径
  - `src/zot-cli/src/commands/doctor.rs` — 无条件调用 `PdfiumBackend::status()` → `pdfium(ProbeOnly)` → 遍历候选并 `bind_to_library`
- 攻击路径：第三方仓库放置同名恶意库 → 用户/Agent 按文档先跑 `zot --json doctor` → RCE

## Requirements

### R1 — 删除 CWD 候选（必须）

- 从 `candidate_library_paths()` **完全移除** `env::current_dir()` 相关候选。
- 不得以任何形式恢复“当前工作目录隐式加载”行为（包括文档建议、fallback、feature flag）。

### R2 — 保留受信任候选（必须）

加载候选仅允许以下来源（顺序可保留现有优先级）：

1. 显式环境变量 `ZOT_PDFIUM_LIB_PATH`（优先）
2. 显式环境变量 `PDFIUM_LIB_PATH`
3. 可执行文件同目录（portable / side-by-side 部署）
4. 受管缓存目录（`ZOT_PDFIUM_CACHE_DIR` 或系统 cache 下的 `pdfium-{version}`）

“无 trust 则拒绝”在本任务中的可执行含义：只尝试上述信任候选；候选均失败且不允许下载时返回现有 `pdfium-unavailable` 错误，**绝不回退到 CWD**。

### R3 — 禁止裸库名 system fallback（必须）

- 删除 `Pdfium::bind_to_system_library()` / `bind_pdfium_from_system()` 回退。
- 任何 Pdfium 动态加载都必须使用上节列出的路径限定候选；不得把平台裸库名交给默认动态库搜索器。
- 依赖系统安装 Pdfium 的用户必须通过 `ZOT_PDFIUM_LIB_PATH` / `PDFIUM_LIB_PATH` 显式提供文件或目录。

### R4 — doctor / ProbeOnly 行为（必须）

- `PdfiumBackend::status()` / `ProbeOnly` 不得再因 CWD 中存在同名库而判定 `available=true` 或触发对该路径的加载。
- doctor 本身逻辑无需大改：修在 discovery 层即可惠及所有调用方。

### R5 — 测试与回归（必须）

- 单元测试断言 `candidate_library_paths()` 结果严格等于显式 env、可执行文件同目录与受管缓存三个允许来源，不含隐式 CWD 或裸库名来源。
- 回归测试不得修改进程级环境变量或 CWD，避免并行测试竞争和为测试新增 `unsafe`。
- 现有 Pdfium 相关单测继续通过。

### R6 — 文档/注释一致性（必须，最小）

- 更新 `candidate_library_paths` 内注释，去掉“兼容当前工作目录”表述。
- 若公开文档明确声称“可把 Pdfium 放在当前目录”，同步修正为 env / 可执行文件旁 / 受管缓存；**不**借机重写 README 大段。

## Out of Scope

- Pdfium 下载 SHA256 校验与原子安装（→ `07-26-fix-pdfium-download-verify`）
- 显式 env / executable-adjacent 路径的所有者与可写权限 hardening
- 拆分 `NativeArtifactResolver` / 完整 trust policy 架构
- doctor 命令 UI/文案大改、MCP、release notes 全文（父任务收尾时再写 CHANGELOG 安全条目即可；本任务可在 Notes 记一笔）

## Constraints

- 不引入 `unsafe`；回归测试也不得为隔离环境变量新增 `unsafe`
- 不使用 `unwrap()` / `todo!` / `dbg!`（workspace lint）
- 最小 diff：只动 discovery 与测试/必要注释
- 回滚策略：用户应改用 `ZOT_PDFIUM_LIB_PATH`；**禁止**通过回滚恢复 CWD 候选

## Acceptance Criteria

- [x] `candidate_library_paths()` 源码中无 `current_dir` 候选逻辑
- [x] 加载链无 `bind_to_system_library()` 或其他裸库名 fallback
- [x] 单元测试 `candidate_library_paths_only_uses_trusted_sources` 通过：候选严格等于三个允许来源，且测试不修改全局 env/CWD
- [x] `cargo test -p zot-local` 相关 Pdfium 测试全绿
- [x] `cargo clippy -p zot-local --all-targets -- -D warnings` 通过
- [x] 注释/最小文档不再暗示 CWD 是合法 Pdfium 落点

## Dependencies

- 无前置子任务；本任务是整棵 audit-remediation 树的第一项。
- 后续 `fix-pdfium-download-verify` 依赖本任务先收敛候选路径集合。

## Notes

- 父任务：`07-26-audit-remediation`
- 报告行号已校正到当前代码；实现以 `src/zot-local/src/pdf.rs` 为准，不以报告旧行号为准。
- 验收证据（2026-07-26）：聚焦回归 1/1、`zot-local --lib` 41/41、`just ci` 全门禁通过。
