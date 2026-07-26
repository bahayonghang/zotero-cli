# Design: 工程化与文档基线

## 1. 门禁分层

`justfile` 将写操作和检查操作分开：

```text
version-sync ─┐
skills-sync  ─┴─ explicit mutation / install only

version-check ─┐
fmt/check/clippy/test ├─ ci-check <- ci compatibility alias
skills-check  ────────┘
```

`skills-check` 只运行 mirror byte comparison 与 fixture tests，不依赖 `_install-skills`。
`install` 仍调用私有同步 recipe。GitHub matrix 运行 `just ci-check` 后再执行
`git diff --exit-code`；后者只属于 clean checkout CI，不放进本地 `just ci`，否则实现阶段
正常的未提交代码会被误判为门禁改写。

## 2. CI 拓扑

- `stable`：三 OS matrix，安装 stable + fmt/clippy + just，运行 `just ci-check` 和 clean diff。
- `msrv`：Ubuntu + Rust 1.85.0，运行 `cargo check --workspace --locked`。
- `security`：Ubuntu 上运行 `cargo audit` 与 `cargo deny check`。
- `unused-deps`：stable 运行 `cargo machete`；固定 nightly 运行
  `cargo udeps --workspace --all-targets`。nightly 仅用于 udeps，不成为编译支持承诺。

依赖工具使用成熟 setup/action 或锁定的 cargo install 版本，避免 `@main`。workflow
本身不执行发布和外部写操作。

## 3. MSRV 收敛

- `pdfium-render` 关闭默认 features，只保留当前使用的 `pdfium_latest` 与 `thread_safe`，
  从图中移除未使用且要求 1.88 的 `image`。
- `libloading` 的 upstream 约束是宽松 `0`，通过 `Cargo.lock` 精确锁到 0.8.9。
- ICU 传递依赖锁到 2.1.x（MSRV 1.83）；所有锁定都由
  `cargo +1.85.0 check --workspace --locked` 给出最终证据。
- direct/transitive vulnerable dependencies升级到最小已修补兼容版本；不添加 advisory ignore。

Pdfium render API 本任务不改，关闭 image feature 前以源码使用搜索和 `zot-local` 测试证明
没有调用 image conversion API。

## 4. Lint 收敛

每个 member manifest 添加：

```toml
[lints]
workspace = true
```

`workspace_version_guard` 从根 `[workspace].members` 遍历 member manifest 并断言上述结构，
同时根据 `[workspace.package].version` 检查 CHANGELOG heading。现有内部 path 依赖断言保留。

启用 lint 后只修实际 failure：测试 `unwrap()` 替换为带上下文 `expect()`；Pdfium cache-dir
测试把 `pdfium_cache_dir()` 抽成接受可选 override 的纯 helper，生产 wrapper 读取 env，测试
直接传路径，从而删除测试中的 unsafe 环境修改。

## 5. cargo-deny 策略

新增 `deny.toml`：

- advisories/yanked 默认拒绝，不忽略已知漏洞；
- allow list 仅列当前依赖图所需的 OSI/项目已接受许可证；
- duplicate versions 保持 warning，避免把无关全图升级混入本任务；
- registry 只允许 crates.io，未知 registry/git source 拒绝。

先跑分项 `cargo deny check advisories`、`licenses`、`bans`、`sources` 定位配置，再跑完整
`cargo deny check`。配置注释记录例外理由；没有理由的 license exception 不加入。

## 6. 文档与回滚

`AGENTS.md` 更新运行事实；CHANGELOG 1.0.0 汇总父任务已落地的安全与可靠性变化，包含
API key 轮换提醒。工程契约写入 `.trellis/spec/zot-cli/backend/quality-guidelines.md`。

回滚可以按语义整体撤销 workflow/justfile/manifest/guard/docs；没有数据迁移。若某个依赖
升级引发 API 变化，优先选仍满足修补与 MSRV 的兼容版本，不扩大到业务代码重写。
