# Design — 本地分类创建

## CLI proposal

collection create NAME [--parent KEY] --write-backend local
确认追加 --confirm --expected-server-id ID --expected-library-version N。

为创建参数使用独立确认结构，不把不存在对象的 version 填成 0 来复用 LocalWriteArgs。Web 默认参数/行为保持原样；Web 拒绝 local-only flags。

## Consistent preview

通过 LocalApiClient 获取当前库分类集合及同一响应的本地 Last-Modified-Version，进行同级精确名称与父 key 校验，展示根/父分类路径。
分类集合须完整读取，不能用默认 limit 导致漏掉重复名。若响应分页，遍历并核对库版本一致；变化则返回冲突，不无限重试。
父对象的可用性及库归属由同库 API 读取校验；不使用 SQLite 或 Web 的版本。

确认时再次取得集合与库版本，要求与预览一致，再授权。POST 同时带实例 ID 与 If-Unmodified-Since-Version，封住校验后发生变更的窗口。
任何库级变动可能使创建预览失效，这是首版有意采用的保守条件，不静默刷新版本继续写入。

## Transport

新增 local collection-list snapshot 和 create_collection 方法，职责留在 zot-desktop：

- GET 当前库 collections，读取对象与本地库版本。
- POST 当前库 collections，单元素数组，只发 name/parentCollection。
- 精确解析成功/unchanged/failed 索引，校验仅索引 0，并验证返回对象身份；不能只看 HTTP 200。
- 成功取得 key 后 GET 回读 name、parentCollection、server。
- 版本化 POST 不混用 Write-Token；库版本条件已负责并发保护。未知提交不重发。

## Result

沿用 backend/state/server_id/library/write_accepted/verified/error，增加 operation=create-collection、collection_key（未获知时为空）、expected_library_version、before=null、after={name,parentCollection}。
重复名作为明确输入/目标冲突，附候选 key，不复用已有对象。
200 中 failed 是失败；可靠成功对象 + 正确回读才是 applied；accepted 但不匹配为 verification-failed。超时/5xx/不可靠响应为 unknown，不凭后来发现的同名分类认领此次创建。

## Technical verification before coding

先验证官方协议在目标 Zotero 版本上的 list/header 和对象级 create 响应；有 mock 和独立桌面验收覆盖。若缺少可用本地库版本或返回协议不同，停在规划修订；禁止移除条件保护。
本轮不需要启动 Zotero 或写真实主库来完成规划。

## Files / compatibility

主要涉及 zot-desktop/local_api.rs 和 tests、CLI args、collection.rs、一个小的本地 create 编排模块或现有 local_write 合适位置、文档/skill/spec。仅在创建与已有执行共享代码确有重复时提取授权辅助函数，不泛化整个 mutation engine。
