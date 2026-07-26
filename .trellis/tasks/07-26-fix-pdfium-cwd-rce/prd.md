# P0热修:移除Pdfium CWD加载路径

## Goal

关闭审计报告 P0-01 / QW-01：`candidate_library_paths()` 将当前工作目录中的同名 Pdfium 动态库纳入加载候选，导致在不可信目录执行 `zot doctor`（以及任何触发 Pdfium probe/load 的命令）时可能加载恶意 `pdfium.dll` / `libpdfium.so` / `libpdfium.dylib` 并执行任意 native code。

本任务是 security hotfix，范围严格收敛为 **删除隐式 CWD 候选 + 锁定 trust 候选顺序 + 安全回归测试**。下载校验、原子安装、executable-adjacent 所有者检查等完整 hardening 归属后续子任务（`fix-pdfium-download-verify` 等），不在本任务实现。

## Background / Evidence

- 审计条目：`zotero-cli-code-audit-2026-07-25.md` §1.1 P0-01、§6 QW-01
- 核实状态：CONFIRMED（dev@39aaeb4）
- 当前代码：
  - `src/zot-local/src/pdf.rs` — `candidate_library_paths()` 含 `env::current_dir().join(library_name)`
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
5. 系统库绑定（现有 `bind_pdfium_from_system()` 路径，不经 candidate list）

“无 trust 则拒绝”在本任务中的可执行含义：只尝试上述信任候选；候选均失败且不允许下载时返回现有 `pdfium-unavailable` 错误，**绝不回退到 CWD**。

### R3 — doctor / ProbeOnly 行为（必须）

- `PdfiumBackend::status()` / `ProbeOnly` 不得再因 CWD 中存在同名库而判定 `available=true` 或触发对该路径的加载。
- doctor 本身逻辑无需大改：修在 discovery 层即可惠及所有调用方。

### R4 — 测试与回归（必须）

- 单元测试断言 `candidate_library_paths()` 结果 **永不包含** `current_dir().join(library_name)`。
- 回归夹具：在临时 CWD 放置同名 decoy 库文件时，候选列表仍不含该路径（即使文件存在）。
- 现有 Pdfium 相关单测继续通过。

### R5 — 文档/注释一致性（必须，最小）

- 更新 `candidate_library_paths` 内注释，去掉“兼容当前工作目录”表述。
- 若公开文档明确声称“可把 Pdfium 放在当前目录”，同步修正为 env / 可执行文件旁 / 受管缓存；**不**借机重写 README 大段。

## Out of Scope

- Pdfium 下载 SHA256 校验与原子安装（→ `07-26-fix-pdfium-download-verify`）
- executable-adjacent / system 路径的所有者与可写权限 hardening
- 拆分 `NativeArtifactResolver` / 完整 trust policy 架构
- doctor 命令 UI/文案大改、MCP、release notes 全文（父任务收尾时再写 CHANGELOG 安全条目即可；本任务可在 Notes 记一笔）

## Constraints

- 不引入 `unsafe`
- 不使用 `unwrap()` / `todo!` / `dbg!`（workspace lint）
- 最小 diff：只动 discovery 与测试/必要注释
- 回滚策略：用户应改用 `ZOT_PDFIUM_LIB_PATH`；**禁止**通过回滚恢复 CWD 候选

## Acceptance Criteria

- [x] `candidate_library_paths()` 源码中无 `current_dir` 候选逻辑
- [x] 单元测试 `candidate_library_paths_never_includes_cwd`（或等价名）通过：候选不含 CWD 路径
- [x] 在 CWD 放置平台同名 decoy 文件时，候选列表仍不含该 decoy 路径（list 构造不依赖 exists；回归测清除 env 后断言 CWD join 不在候选中）
- [x] `cargo test -p zot-local` 相关 Pdfium 测试全绿
- [x] `cargo clippy -p zot-local --all-targets -- -D warnings` 通过
- [x] 注释/最小文档不再暗示 CWD 是合法 Pdfium 落点

## Dependencies

- 无前置子任务；本任务是整棵 audit-remediation 树的第一项。
- 后续 `fix-pdfium-download-verify` 依赖本任务先收敛候选路径集合。

## Notes

- 父任务：`07-26-audit-remediation`
- 报告行号已校正到当前代码；实现以 `src/zot-local/src/pdf.rs` 为准，不以报告旧行号为准。
