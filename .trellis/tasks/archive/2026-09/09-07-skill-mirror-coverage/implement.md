# 执行计划

1. 批准后先增加第二 skill 漂移的真实默认入口回归，观察修改前失败。
2. 在默认检查层枚举 canonical skill，保留显式接口和 compare_trees；不新增依赖。
3. 添加全未装、部分安装、新增 skill、无写入和无关 skill 不影响的场景。
4. python -m unittest discover -s scripts/tests -p "test_*.py"。
5. 运行默认脚本及两个显式 skill 检查；在临时目录验证全缺失 exit 0、部分缺失 exit 1。
6. 更新 README.md 验证段，执行 just ci、git diff --check；强模型复核 AC1–AC3。

不靠同步镜像掩盖失败；与 C1 并行时 C1 独占 canonical skill，C2 独占检查脚本。

## 精确执行文件清单
- scripts/check_skill_mirrors.py：默认全 skill 覆盖及保留的显式模式。
- scripts/tests/test_check_skill_mirrors.py：真实默认入口的独立回归。
- justfile：只有默认命令调用需要调整时修改，不加入同步行为。
- README.md、README.zh-CN.md：两语种验证段一并更新。
