# Implement: 移除 Pdfium CWD 加载路径

## Checklist

1. [x] 编辑 `src/zot-local/src/pdf.rs`：删除 `current_dir` 候选块，更新步骤 2 注释
2. [x] 同文件测试模块新增 `candidate_library_paths_never_includes_cwd`
3. [x] 运行 `cargo test -p zot-local pdf -- --nocapture`（或等价 filter）
4. [x] 运行 `cargo clippy -p zot-local --all-targets -- -D warnings`
5. [x] 确认无 README 需改（已 grep）；若有遗漏一并改
6. [x] `trellis-check` / 质量门通过后准备 commit

## Validation commands

```bash
cargo test -p zot-local candidate_library_paths -- --nocapture
cargo test -p zot-local --lib
cargo clippy -p zot-local --all-targets -- -D warnings
```

可选全量：

```bash
just ci
```

## Review gates

- Diff 仅触及 `pdf.rs`（+ 若必要的极小文档）
- 源码与测试均无 `current_dir` 作为 candidate 来源
- 不改动 download / extract / doctor 命令结构

## Rollback

- **正确回滚**：用户文档指引改用 `ZOT_PDFIUM_LIB_PATH`；代码保持删除 CWD
- **错误回滚**：恢复 `current_dir` 候选 — 禁止

## Commit message draft

```
fix(zot-local): stop loading Pdfium from the current working directory

Remove the CWD candidate from Pdfium discovery so `zot doctor` and other
probes cannot bind a malicious same-named library planted in an untrusted
project directory. Explicit env paths, executable-adjacent, and managed
cache candidates remain. Add a regression test that candidates never
include current_dir.
```
