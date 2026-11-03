# Implementation plan — 本地分类创建

## Activation / dependency

- [x] 整组规划与本子任务获得审阅批准；A 已稳定后再修改共享 CLI/授权代码。
- [x] 读取 local-api spec、错误与日志契约及 design 的技术核查项。

## Steps

1. [x] 核实 Local API collection-list 的库版本与完整列表协议；mock 捕获空库、分页/变化及缺 header。
2. [x] 添加集合快照和 POST 创建客户端，严格解析逐对象结果，测试实例/库边界、失败、超时与不重发。
3. [x] 扩展 collection create 的 local 参数/预览/确认；同级重名、空白名称、父级错误在授权前拒绝。
4. [x] 验证根分类也使用库版本条件，预览后父分类删除/同名新建/其他库内变动导致冲突。
5. [x] 确认回读及 report，保留 Web create 回归和真实进程参数检查。
6. [x] 更新双语文档、canonical skill、local-api spec；创建后的 key 用于已有 add-item 示例。
7. [x] 独立桌面库真实创建根/子分类、检查 UI、重启后核对；提交不确定的故障用 mock 并明确证据边界。已完成，见父 validation.md。

## Checks

- cargo test -p zot-desktop --locked
- cargo test -p zot-cli --locked
- just ci
- npm --prefix docs run build
- git diff --check

skill 修改后有意生成镜像并运行检查；不以 HTTP 200 替代业务断言。

## Stop / evidence

库版本或 POST 行为若违背 design 先修订规划。报告持久化不涉及真实密钥。validation.md 映射 C-AC1-C-AC6；创建成功不意味着未来请求可以按同名自动重试。
