# 本地内容操作（Zotero 10+）

开发分支验收状态：本地标签、分类创建和 `library organize` 已通过自动化验证，以及 Windows Zotero 10.0.7 独立 profile/data 的授权、核心写入、UI 和正常重启回读验收。组库、其他操作系统和真实提交中断尚未实测；故障注入结果属于自动化证据。

`item update`、分类成员操作、单条 `item tag add/remove`、`collection create` 和单 key `collection delete` 支持官方 Local API。Zotero 必须运行并在高级设置启用本地通信。无需 Web API key 或额外插件；首次确认写入时，Zotero 会请求用户授权。

先运行 `zot --json doctor` 查看 `local_api_write`。supported 表示版本/协议支持，credential_present 表示缓存授权；doctor 不弹授权框、不试写，verified 始终为 false。

## 预览与确认

下列 key 均是示例，请替换成真实八位 item/collection key：

```bash
zot --json item update ITEMABCD --write-backend local --field DOI=10.1000/example
zot --json item tag add ITEMABCD --tag 待读 --tag "C++/安全" --write-backend local
zot --json item tag remove ITEMABCD --tag 过期 --write-backend local
zot --json collection create "待读" --write-backend local
zot --json collection create "子类" --parent ROOT2345 --write-backend local
zot --json collection add-item COLLB234 ITEMABCD --write-backend local
zot --json collection remove-item COLLA234 ITEMABCD --write-backend local
zot --json collection move-item ITEMABCD --from COLLA234 --to COLLB234 --write-backend local
zot --json collection delete COLL0001 --write-backend local
```

以上默认只返回 preview，或没有变化时返回 unchanged。检查 before/after/skipped_fields 后，保持同一参数再确认。条目和成员操作增加 `--confirm --expected-server-id <server_id> --expected-version <expected_version>`。`collection create` 改用 `--expected-library-version`。值必须取自该次预览，不能使用 Web API/SQLite 的版本号。发生冲突时重新预览。

移动只移除源分类、加入目标分类；其他归属保留，不复制条目或移动附件文件。仅支持同库顶层普通条目；目标由参数决定，不跟随 UI 当前选择。move-item 必须显式选择 local。

## 单条标签

`item tag add/remove` 不带 `--write-backend local` 时仍立即走 Web API。`item tag batch` 不接受 local 后端，也不能用来代替单条本地标签。

- 新标签是手工标签（type 0）。精确同名已存在时返回 unchanged，不改变原 type，包括自动标签 type 1。
- 移除按精确名称删除该名称的全部关联。同名不同 type 会一并移除，并在 preview 的 before/after 中完整展示。
- 未点名的标签对象及其未知属性保留。条件 PATCH 的 body 只含完整 tags 数组和本地 version。
- 名称按输入保留，包括中文、空格和特殊字符。不做大小写、Unicode 或同义词折叠，也不修剪非空名称。纯空白拒绝。
- 回读按标签多重集合比较：数组顺序不同不算失败；type 缺省与 0 等价，type 1 必须区分。
- 只作用于同库、非回收站的顶层普通条目。不能用 `--field tags` 绕过这个契约。

## 创建分类

`collection create` 不带 `--write-backend local` 时仍走 Web API。重命名和重挂父级没有本地写路径。单 key `collection delete` 可以显式走本地预览/确认。

```bash
zot --json collection create "待读" --write-backend local --confirm --expected-server-id <server_id> --expected-library-version <library_version>
```

确认使用库版本，不使用条目 version，也不要把不存在对象的 version 填成 0。预览会读取完整分类列表和同一响应的 `Last-Modified-Version`。同一父级精确同名会拒绝并列出候选 key，不复用、不合并、不重命名；不同父级可以同名。只创建根分类，或同库已有父分类下的一层子分类。

POST 是单元素数组，只含 name 和 parentCollection，并带实例 ID 与 `If-Unmodified-Since-Version`。HTTP 200 不等于成功；只有对象级成功且回读 name、parent、server 一致才是 applied。超时、5xx 或不可靠响应是 unknown，不会自动重发，也不会凭事后同名分类认领成功。创建流程不改成员、children 或附件。返回的 key 可作为现有 `collection add-item` 的输入。

## 删除分类

`collection delete KEY` 不带 `--write-backend local` 时仍立即走 Web API。显式 local 时默认只预览，不读取授权，也不发送 DELETE。

```bash
zot --json collection delete COLL0001 --write-backend local --confirm --expected-server-id <server_id> --expected-version <version> --expected-descendants-sha256 <sha256>
```

预览列出 Local API 分类列表返回的目标分类和全部子孙分类，并给出按 key 排序后的子孙集合 SHA-256。响应里带 `deleted` 时，这些子孙会计入列表和哈希。Zotero 10.0.8 的 `GET /collections` 不返回已在废纸篓中的分类，也不识别 `includeTrashed`，所以它们不会出现在预览或哈希里；本命令不打开 SQLite 去补齐。确认绑定的是分类对象 version，不是库版本；预览里的 0 可以确认，不要自行填 0。Server ID、对象版本或子孙哈希任一不符就拒绝，不会授权或删除。

删除会永久擦除该分类和每一个子孙分类，包括列表里看不到的废纸篓子分类，不会把子分类提升到父级。文献留在库里，只失去这些分类的成员关系。只发送这一把 key 的 DELETE，不调用批量删除。

只有 DELETE 返回 204 且 Server ID 一致，随后 GET 404，才是 applied 且 verified。HTTP 200、超时、5xx 或 Server ID 不符是 unknown，不能再用后来的 404 改记为成功。204 之后对象仍在是 verification-failed。DELETE 本身 404 是 absent，这次调用没有删除它。这些状态即使 envelope 为 ok 也不是成功，不得盲重试。

## 字段和作者

字段按当前 itemType/schema 校验，默认仅填空。若要替换非空值，预览时就添加 `--overwrite`，确认时保持该参数。空值清除和 itemType 转换不在本功能范围内。

作者通过 `--creators-file authors.json` 指定完整有序列表，例如：

```json
[
  {"creatorType":"author","firstName":"Ada","lastName":"Lovelace"},
  {"creatorType":"author","name":"Research Institute"}
]
```

已有作者列表需要 --overwrite 才替换；不会自动拼接、拆分姓名或去重。citationKey 是当前 schema 支持时可显式编辑的字段，不能用它替代 item key；Better BibTeX 的自动重算不由本命令保证。

## 结果和边界

- applied + verified=true：写响应和回读均支持成功。
- unknown：提交结果未知，即使 observed_expected_state=true 也不能证明此次请求成功，不能盲重试。
- verification-failed：write_accepted=true，但回读失败或目标值不匹配。
- 冲突、无权限、拒绝授权和无效输入返回标准错误 envelope。

Zotero 的一次授权 key 只用于当前进程；选择记住授权时，CLI 按 profile 和 Server ID 保存到平台配置目录的 local-api-credentials.json，不包含在 config show 输出。该目录由 AppConfig::config_dir() 确定，Windows 通常为 `%AppData%\zot`。

既有命令默认仍走 Web；local 专用参数不能用于 Web。任何本地失败都不会自动切换后端。笔记、附件、`item tag batch`、分类重命名/重挂父级和联网自动补全尚未接入本地写路径。SQLite 始终只读。
