# C2 Design

归一化放在统一平台匹配边界，显式映射三组已证实别名，不新建通用平台注册系统。既有Codex虚拟平台与解析路径保留。未知平台仍按既有规则处理，不借机扩展支持。

正式测试两层：纯函数验证别名/配置；subprocess调用get_context --mode phase --step 2.1/2.2 --platform <id>，同时核对exit0与有效工作流条款。与canonical名称等价不足以单独证明成功，还需正文包含该模式的实施/检查角色与规则，防止双方都为空。

测试使用临时/显式配置或纯解析器，不调用task.py start、不修改用户配置、不启动模型。由现有unittest discovery接入just skills-check，避免新增检查框架。

本仓库Trellis0.7.0-beta.3生成文件可局部修改；不改上游/npm或hash基线。C3在harnesses.md记录ID、显示名、复核命令与升级后回归点。任何超出已知别名的解析错误单独报告。
