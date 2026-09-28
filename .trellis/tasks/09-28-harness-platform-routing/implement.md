# C2 Implementation（2026-09-28 已批准）

1. 读取批准任务、workflow_phase.py与实际CLI调用；记录修前探针。
2. 新增scripts/tests/test_harness_platform_routing.py；先运行聚焦测试，记录三组别名修前失败。
3. 在workflow_phase.py加入最小统一别名映射；不改变工作流文本或策略配置。
4. 必须通过：python -m unittest discover -s scripts/tests -p test_harness_platform_routing.py；python -m unittest discover -s scripts/tests -p test_*.py；just ci；git diff --check。
5. 检查真实五平台2.1/2.2输出与Codex配置组合。至少一项断言检查条款内容，不能只检查returncode/字符串长度。
6. 强模型审查没有上下文扩大、未知平台误归一化或批准门变化；把已验证契约交给C3写回。

Write files: .trellis/scripts/common/workflow_phase.py；scripts/tests/test_harness_platform_routing.py。不改本机ignored adapters。
