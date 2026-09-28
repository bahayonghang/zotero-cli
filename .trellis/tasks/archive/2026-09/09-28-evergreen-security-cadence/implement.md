# C4 Implementation（2026-09-28 已批准）

1. 确认C1安全门通过，C3共享说明已落定。
2. 仅修改ci.yml的周度触发、非security job条件和security只读权限；保留其他事件与命令。
3. 必须通过actionlint；人工逐项检查push/PR/manual/schedule四类事件的job集合、权限、needs和失败传播。审核不得由YAML语法通过代替。
4. 必须通过cargo audit、cargo deny check、just ci、npm --prefix docs run build、git diff --check。
5. 更新harnesses.md和quality-guidelines的安全门、时间敏感证据与五工具适用说明。
6. 强模型审查没有Pages部署、权限扩大、自动修锁或忽略漏洞。真实scheduled运行保持UNVERIFIED，另获远端交付授权后在新SHA记录。

Write files: .github/workflows/ci.yml；docs/agents/harnesses.md；.trellis/spec/zot-cli/backend/quality-guidelines.md。低成本模型可执行冻结后的YAML/文案，强模型负责事件矩阵与验收。
