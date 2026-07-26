# Design: 移除 Pdfium CWD 加载路径

## Problem

`PdfiumBackend::pdfium()` 按 `candidate_library_paths()` 返回的顺序对**存在的**路径调用 `Pdfium::bind_to_library()`。修复前审计基线的候选第 4 项是：

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
| 系统库裸名 (`bind_to_system_library`) | 平台默认搜索路径可能包含 CWD | **删除** |
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

4. 删除候选遍历后的 `bind_pdfium_from_system()` 回退。`pdfium-render 0.9.0` 的该 API 使用 `Library::new()` 加载平台裸库名，无法保证绕过 CWD；保留它会使显式候选修复失效。

`status()` / `doctor` 不改控制流；它们继续消费 discovery 结果。`pdfium()` 在可信候选失败后直接进入受管下载（`AllowDownload`）或返回现有 `pdfium-unavailable`（`ProbeOnly`）。

### Tests

同文件 `#[cfg(test)]` 模块新增：

1. **`candidate_library_paths_only_uses_trusted_sources`**
   - 调用生产 `candidate_library_paths`
   - 独立构造显式 env、executable-adjacent、managed cache 三类允许候选
   - 断言实际列表与允许列表严格相等，从而捕获新增 CWD 或其他隐式来源
   - 不清空 env、不切换 CWD，避免进程级状态竞争及 Rust 2024 `unsafe` env mutation

2. 静态审阅确认加载链不再调用 `bind_to_system_library()`；聚焦测试覆盖候选 trust policy，clippy/全量测试覆盖控制流编译与既有行为。

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
| 仅靠系统裸库名搜索 | 可能可用 | 不再尝试；需显式设置 env 路径 |

破坏性：故意、安全必要。不提供兼容开关。

## Security residual risk (accepted for P0)

- 恶意库仍可通过 **显式 env** 加载（用户 intentional）。
- **exe-adjacent** 若安装目录可被他人写，仍可劫持（完整 hardening 后续）。
- **managed download** 仍无 checksum（P1 `fix-pdfium-download-verify`）。

P0 成功标准：隐式 CWD candidate 从 1 → 0，裸库名动态加载调用从 1 → 0。

## Rollout

- 随 dev 分支提交；可单独 cherry-pick 为 security hotfix。
- CHANGELOG 安全条目可在本任务或 engineering-baseline 统一写；本任务至少在 task notes 记录。
