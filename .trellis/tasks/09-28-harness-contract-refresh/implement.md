# C3 Implementation（2026-09-28 已批准）

1. 确认C2通过，读取最新平台结果与能力来源。
2. 在.trellis/config.yaml显式设codex.dispatch_mode: inline；修改共享说明中已证实冲突。
3. 更新五工具矩阵与证据日期、模型选择和交接规则；保留未验证项。CLAUDE.md无需修改。
4. 修正README.md/README.zh-CN.md首屏Desktop合并说法，遵从WebMergeWriter。
5. 必须通过：C2正式回归；python .trellis/scripts/get_context.py --mode phase --step 2.1 --platform codex（输出inline实际步骤）；just ci；npm --prefix docs run build；git diff --check。
6. 人工核对AGENTS、薄入口、显式配置、workflow、canonical skills与矩阵；确认Grok/Kimi有支持≠已验证自动注入。
7. 强模型验收后将各适用工具和新证据写入正式说明。新会话/hook/权限仍未验证，不伪造PASS。

Write files: AGENTS.md、docs/agents/harnesses.md、.trellis/config.yaml、README.md、README.zh-CN.md。无需skills-sync、native配置复制或付费新会话。
