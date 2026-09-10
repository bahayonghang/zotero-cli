# 设计

修改文件：.github/workflows/ci.yml 添加独立 docs build job，README.md / README.zh-CN.md 验证段、docs/agents/harnesses.md 验证边界。不调整现有 Rust jobs，不修改部署环境，默认不新增依赖/改 lock，不自动增加定时 workflow。

R1：PR 触发的 CI 已存在，在相同 workflow 加 docs job，沿用 Deploy docs 的 checkout/setup-node/npm ci/build；不包含 Pages Configure/Upload/Deploy，也无 pages:write/id-token:write。R2：Node 版本与 deployment 当前约定一致；Node 生命周期升级若需依赖变动另提范围，不能夹带。R3：当前 5b53e7f 的 hosted CI 已全绿，历史失败只留防回归背景。

R2/AC2 的具体权限机制：在新增 docs job 显式设置 job-level `permissions: { contents: read }`；其余 token scope 不授予，禁止继承仓库默认写权限。验收时人工读回该 job 的 permissions 键值并检查没有其他 write scope，再运行 actionlint；actionlint 成功本身不能证明最小权限。现有 Rust jobs 的权限不在本任务重构范围。

AC1 先在已有依赖或获准 npm ci 的环境构建基线，再在临时副本引入一个确定的 VitePress 错误并确认非零；真实源码不留故障。AC2 比较 lock/源码 diff 并 actionlint。AC3 最终 just ci 一次，记录新增 docs job 的同 SHA hosted 结果；未发布 PR/触发 CI 的情况保留 UNVERIFIED。

便宜模型负责已有模式下新增 job 和说明；Codex/Claude Code 强模型复核权限、触发、版本一致和环境/源码失败归因。本子任务的批准范围包含执行现有 lock 的 npm ci 以准备验证环境，不新增或升级依赖；不包含外部发布授权。回退新 job 与文档即可。
