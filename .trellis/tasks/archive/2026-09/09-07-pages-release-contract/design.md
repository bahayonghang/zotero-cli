# 设计

## 根因与选择
现有 docs 构建在失败 run 中已经成功，部署尚未开始即被环境保护规则拒绝；同 SHA 的 main 手动部署成功。修复归属是发布标签规范，不是修改 VitePress 或放宽 Pages 保护。

最小修改文件：新增 docs/agents/release.md，README.md / README.zh-CN.md 增加发布说明链接。现有 .github/workflows/deploy-docs.yml 无需为该根因改代码；不新增重复 tag 验证规则引擎。版本示例使用 Cargo.toml 的 version 生成 v 前缀，不改变 Cargo 格式。

R1/AC1 通过审查 Cargo 版本、发布命令草稿与实际 branch/tag policy 来验证。R2/AC2 通过授权分阶段说明与本地 diff 证明；R3/AC3 以部署 run 关联的 tag/SHA/build/deploy 结果作为未来 hosted 证据。仅 workflow_dispatch main 成功不覆盖 release tag 路径。

## 交付与权限
本地文档交付可完成并获审查；release 路径故障的真实闭环保持 UNVERIFIED，直到用户另外授权并完成合规发布。不得为了关任务伪造或主动触发 release。现有 1.0.0 标签保持不动。远端环境改 main/v* 为更宽规则是未选方案，不在批准范围。

Claude Code/Codex 强模型审查归属与权限，便宜模型执行精确文档和命令示例。所有五套 harness 共用这份契约。回退文档即可。
