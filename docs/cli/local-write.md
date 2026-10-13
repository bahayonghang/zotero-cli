# 本地内容操作（Zotero 10+）

`item update` 和分类成员操作支持官方 Local API。Zotero 必须运行并在高级设置启用本地通信。无需 Web API key 或额外插件；首次确认写入时，Zotero 会请求用户授权。

先运行 `zot --json doctor` 查看 `local_api_write`。supported 表示版本/协议支持，credential_present 表示缓存授权；doctor 不弹授权框、不试写，verified 始终为 false。

## 预览与确认

下列 key 均是示例，请替换成真实八位 item/collection key：

```bash
zot --json item update ITEMABCD --write-backend local --field DOI=10.1000/example
zot --json collection add-item COLLB234 ITEMABCD --write-backend local
zot --json collection remove-item COLLA234 ITEMABCD --write-backend local
zot --json collection move-item ITEMABCD --from COLLA234 --to COLLB234 --write-backend local
```

以上默认只返回 preview，或没有变化时返回 unchanged。检查 before/after/skipped_fields 后，保持同一参数，增加 `--confirm --expected-server-id <server_id> --expected-version <expected_version>`。值必须取自该次预览，不能使用 Web API/SQLite 的版本号。发生冲突时重新预览。

移动只移除源分类、加入目标分类；其他归属保留，不复制条目或移动附件文件。仅支持同库顶层普通条目；目标由参数决定，不跟随 UI 当前选择。move-item 必须显式选择 local。

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

既有命令默认仍走 Web；local 专用参数不能用于 Web。任何本地失败都不会自动切换后端。标签、笔记、附件、批量操作和联网自动补全尚未接入本地写路径。SQLite 始终只读。
