# Design: 移除 Pdfium CWD 加载路径

## Problem

`PdfiumBackend::pdfium()` 按 `candidate_library_paths()` 返回的顺序对**存在的**路径调用 `Pdfium::bind_to_library()`。候选第 4 项（当前代码）是：

```text
env::current_dir().join(pdfium_platform_library_name())
```

`doctor` 在每次运行时 probe Pdfium，因此仅“进入含恶意同名库的目录并执行推荐的 doctor”即可完成加载。CWD 对 CLI/agent 工具是典型不可信边界。

## Trust model (P0 minimal)

| 来源 | Trust | 本任务动作 |
|---|---|---|
| `ZOT_PDFIUM_LIB_PATH` / `PDFIUM_LIB_PATH` | 用户显式 opt-in | 保留 |
| 可执行文件同目录 | 安装/分发边界，非任意 repo CWD | 保留 |
| 受管 cache (`pdfium-{version}`) | 应用控制目录 | 保留（校验属 P1） |
| 系统库 (`bind_to_system_library`) | OS 搜索路径 | 保留（独立路径） |
| **当前工作目录** | **不可信** | **删除** |

P0 不实现：路径所有者检查、cache checksum、禁止 world-writable exe dir。那些会扩大 diff 且与 download-verify 任务重叠。

## Code change

### Primary

文件：`src/zot-local/src/pdf.rs`

1. 删除 `candidate_library_paths` 中 `// 2.2 尝试当前工作目录` 整块。
2. 更新步骤 2 注释：只描述 executable-adjacent + managed cache。
3. 候选顺序变为：
   1. env `ZOT_PDFIUM_LIB_PATH`
   2. env `PDFIUM_LIB_PATH`
   3. `current_exe` parent + library name
   4. managed cache path

`pdfium()` / `status()` / `doctor` **不改控制流**——它们已经只消费 candidate list；删除 CWD 后自动失效整条攻击路径。

### Tests

同文件 `#[cfg(test)]` 模块新增：

1. **`candidate_library_paths_never_includes_cwd`**
   - 计算 `cwd.join(library_name)`
   - 调用 `candidate_library_paths`
   - 断言结果中无该路径（canonical 比较用 path equality；必要时也排除 `cwd` 前缀误伤：只比完整候选 path）

2. **`candidate_library_paths_ignores_decoy_in_cwd`**（更强回归）
   - `tempfile` 目录 + 写入 decoy 同名文件
   - `env::set_current_dir(temp)`
   - 清空/隔离可能污染的 env（`ZOT_PDFIUM_LIB_PATH` / `PDFIUM_LIB_PATH` 若指向 temp 则 remove；测完 restore）
   - 断言 decoy path 不在 candidates
   - `set_current_dir` 还原

注意：Windows 上 `set_current_dir` 与并行测试可能互相干扰。现有测试已用 `unsafe { env::set_var }`；本测试应：

- 尽量短持 CWD 变更
- 用 `defer`-风格 finally 还原（`Drop` guard 或 `scopeguard` 若已有；否则 `let _guard` + 手动 restore，失败路径也要 restore）
- 若 workspace 测试并行导致 flaky，可用 `std::sync::Mutex` 串行化 env/CWD 敏感测试（仅本文件测试内）

更稳妥的实现：**不依赖 `set_current_dir`**，只断言逻辑上 CWD 路径不出现在 list。decoy 文件存在与否不影响 list 构造（list 不检查 exists）。因此 **单测 1 已足够证明 RCE 路径关闭**；decoy 测试可选，作为文档化回归。

推荐：实现单测 1 + 在注释中写明“list 构造阶段即排除 CWD，exists 与否无关”。若时间允许再加 decoy 文件存在性无关的断言（同一测试即可）。

### Docs

- 代码注释必改。
- 公开文档目前未宣称“CWD 放 Pdfium”，grep 无命中 → **无需改 README**。
- 错误 hint 文案已写 “place next to the executable / set env”，保持。

## Compatibility / migration

| 用户行为 | 修复前 | 修复后 |
|---|---|---|
| 把库放在项目 CWD 跑 doctor | 可能加载成功 | 不再加载；需设 env 或放到 exe 旁 |
| 设 `ZOT_PDFIUM_LIB_PATH` | 可用 | 不变 |
| 自动下载 cache | 可用 | 不变 |
| side-by-side 与 `zot` 同目录 | 可用 | 不变 |

破坏性：故意、安全必要。不提供兼容开关。

## Security residual risk (accepted for P0)

- 恶意库仍可通过 **显式 env** 加载（用户 intentional）。
- **exe-adjacent** 若安装目录可被他人写，仍可劫持（完整 hardening 后续）。
- **managed download** 仍无 checksum（P1 `fix-pdfium-download-verify`）。
- **system library** 搜索路径劫持属 OS 层面，本工具不扩大也不收缩。

P0 成功标准：隐式 CWD candidate 从 1 → 0。

## Rollout

- 随 dev 分支提交；可单独 cherry-pick 为 security hotfix。
- CHANGELOG 安全条目可在本任务或 engineering-baseline 统一写；本任务至少在 task notes 记录。
