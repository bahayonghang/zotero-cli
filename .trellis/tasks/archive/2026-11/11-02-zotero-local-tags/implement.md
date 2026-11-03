# Implementation plan — 本地标签

## Activation

- [x] 审阅父/子 PRD、design 和本清单；获得用户批准后 task.py start 本子任务。
- [x] 加载 trellis-before-dev 与 local-api、error-handling、quality、logging specs。

## Steps

1. [x] 添加标签纯差异测试：保留 type=1/0/省略、重复输入/同名标签、中文特殊字符、空白输入、无变化、异常服务端数据。
2. [x] 扩展 args/路由，增加本地标签预览；验证默认仍 Web，local 分支不打开 SQLite/remote。
3. [x] 复用确认执行与标签集合回读；测试最小 PATCH、正确 header、版本/实例冲突、授权拒绝/401、unknown、verification-failed。
4. [x] 添加必要的真实进程 JSON/参数回归，保持既有 async boxing。
5. [x] 更新双语命令/本地写文档、canonical zot skill 与 local-api spec；只宣传单条本地标签。
6. [x] 隔离桌面库真实验收，记录 metadata/children 不变、UI 和重启后的标签类型/内容。已完成，见父 validation.md。

## Checks

- cargo test -p zot-cli local_write --locked
- cargo test -p zot-desktop --locked
- cargo test -p zot-cli --test json_error_contract --locked
- just ci
- npm --prefix docs run build
- git diff --check

有意变更 skill 后运行 just skills-sync；确认镜像检查通过。无需为纯文档行文增加测试。

## Evidence / completion

validation.md 映射 T-AC1-T-AC6，区分 mock、真实 CLI、真实桌面。未获真实写入授权时只完成不依赖授权的检查，并明确尚缺的验收；不能标全部完成。
