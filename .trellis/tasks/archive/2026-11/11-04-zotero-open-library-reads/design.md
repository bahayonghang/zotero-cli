# 读取路径与诊断依据

## 可核对的证据

1. 本机安装包 Zotero 10.0.7 的 app/omni.ja 内，chrome/content/zotero/xpcom/db.js:32 将 DB_LOCK_EXCLUSIVE 设为 true，1593 设置 openNotExclusive: Zotero.isMac，1618 设置主库 locking_mode=EXCLUSIVE。当前上游也记录了连接级锁。上游源码 https://github.com/zotero/zotero/blob/main/chrome/content/zotero/xpcom/db.js 。
2. 独立临时 SQLite fixture：连接 A 设置 locking_mode=EXCLUSIVE，建表插入一行并 commit；A.in_transaction 为 False；只读连接 B 用 mode=ro 查询得到 SQLITE_BUSY；关闭 A 后相同查询返回 1。这是 SQLite 锁机制的复现，没有操作用户库，也没有复现原会话的具体事务。
3. src/zot-local/src/db.rs:1733–1754、2724–2760、2851–2932：只读打开源库、分步 online backup、验证临时快照；More 会重置连续忙计时。5 秒不是整次备份的硬上限。现有 WAL/短事务与持续事务测试保护一致性，不能代替真实桌面兼容验证。历史任务 07-26-fix-sqlite-snapshot 已处理一致性，仍应保留其约束。
4. src/zot-cli/src/commands/item/read.rs:20–32、69–111、113：get 聚合 item/notes/attachments；pdf 在 Pdfium 提取前先解析附件；children 也先开 SQLite。src/zot-cli/src/commands/doctor.rs:31–51 和 src/zot-desktop/src/connector.rs:155–172 只在诊断中探测 /api/。宿主机 zot 1.1.0 实测无 Zotero 进程时 SQLite 可用、HTTP connector-unreachable；沙箱内返回 snapshot-zotero-db / unable to open database file，不能与用户报告的 zotero-db-busy 混用。
5. 官方 Local API 允许在启用本机通信后，无需密钥读取个人库 users/0 或指定群组。附件 /file/view/url 返回纯文本 file URL，本机安装包 server_localAPI.js:1266、1275–1276 也有这条路径。官方协议 https://www.zotero.org/support/dev/web_api/v3/local_api 。

## 建议设计

- 只为 R1、R6–R8 的入口做只读路由。默认对目标明确的当前桌面优先快速探测 Local API v3；进程关闭或服务不可用时，对所配库尝试一次既有快照。禁用 API 且快照可用时仍可成功。两者失败时同时报告原因。对象级 404、身份冲突和文件错误具有明确语义，不跨源重试。完整命令固定在同一个来源，组合 GET 使用库版本变化检测和有界重试，不能将跨提交的结果冒充一致快照。
- 库身份是实现门槛。users/0 仅代表正在运行的实例；启用 profile、显式数据目录等情况下，要先证明与目标相符。10+ 可在已知目标快照中读取 settings 的 localAPI/serverID，与响应头比对，随后每个 GET 发送该 ID；服务器返回 412 即终止。不要为了每次确认先去锁住的库做快照。无已验证绑定时提供显式选择“本次读取运行中的 Zotero”的只读入口，skill 清楚告知所选来源；默认不能偷换指定库。具体入口命名在实现前对照现有 clap 约定。旧版缺少实例 ID 时不承诺自动绑定。
- 在 zot-desktop 复用 loopback 请求构造、禁重定向、scope 映射；普通 GET 不调用 authorize 或读取写密钥。把 API item 和 children 映射到现有 DTO，保持 JSON 及人类输出、字段含义和多 key 行为。先对照现有数据结构；不能推导的字段（例如仅 SQL 可见的内部值）应在实现前列出并复审，不填造假的零值。勿修改通用 local_library() 去接管所有读写调用。
- PDF 继续使用现有附件选择与 Pdfium 提取/缓存逻辑。让 Zotero 返回指定附件的本地 URL，安全转换 Windows 路径（包括空格、中文、转义）；验证本地文件和类型，不跟随网络 URL 或远程重定向。覆盖 stored、linked、多个 PDF、缺失文件。版本不支持此端点时报告该能力边界。
- doctor 保留两种能力及原错误；读命令可以直接执行，无需先跑全套 doctor。skill 仍在首次诊断和 PDF 排障时读取 doctor，但不因 SQLite 单项失败中止已支持的在线命令。仅修改 skills/zot 的权威源和真实需要的镜像/eval。
- 排除拉长超时、裸拷贝主库/WAL、immutable/nolock、强制 checkpoint、长期缓存上次成功快照。前者无法释放连接级锁；中间几项破坏一致性或越过锁；旧快照会漏掉用户刚在窗口里看到的内容。用户未来明确要求修改时，仍按现有后端的授权与版本契约；关闭客户端不能授权直接写 SQLite。

## 找到文献再读取

- src/zot-cli/src/commands/library.rs:61–86 的 library search 通过 run_local 开 SQLite；src/zot-local/src/db.rs:285 起的通用查询同时查元数据、作者、标签和可用的全文索引。刊名命中可能来自摘要。保持通用搜索的现有选项、分页、total 与全文能力报告；显式题名/刊名条件在两个后端都作为字段条件，最终命中后再分页。对仅桌面端暂不支持的组合，说明限制或在能读同库时使用原实现，不能退化成无过滤搜索。
- 本机 10.0.7 的 server_localAPI.js:1132–1135 将 q / qmode 映射到 quicksearch；同安装包 search.js:333–361 将 citationKey 纳入题名作者年份查询，另有 fields 和 everything 模式。服务端 quicksearch 用于缩小候选，再按原字段核对。不要以默认 quicksearch 的零结果判定期刊或 Extra 无匹配。固定数据应验证大小写、空白、题名片段、期刊全称与缩写、摘要/标签命中、正文独有命中和跨页顺序。分页遍历必须有界；未完全遍历时不得报“唯一”“共 N 条”或“无结果”。
- 已安装版本有原生 citationKey 字段。现有命令 library.rs:138–169 先打开 SQLite；db.rs:577–620 只查 Extra 的 Citation Key:/CitationKey: 行，随后 CLI 才尝试 BBT。闭库快照同样要查原生字段；开库走 Local API 字段及 Extra 精确比对，必要时合并可信的插件候选。保持 citation key 值的大小写；按库和 item key 去重，跨条目冲突要求选择。插件生成键若未存入条目且插件不可达，报告来源限制。
- src/zot-remote/src/better_bibtex.rs 的 item.search 是 JSON-RPC POST。仅该只读方法允许作为 GET 规则的例外；本任务的桌面读路径验证 loopback 且禁外部重定向，不能直接信任可覆盖的 ZOT_BBT_URL。BBT 的 libraryID 属于本机内部编号，不能当 group ID；无法证明候选所属库时拒绝自动选择。同名、同 key 跨库 fixture 是门槛。
- skill 将题名/citation key 的单篇目标与批量结果区分：前者定位并继续取信息或 PDF；后者列候选、保留分页，需要阅读哪些条目由请求决定。按期刊请求用 publicationTitle 或 journalAbbreviation 核对。主题走字段中的关键词和标签，避免暗示做了语义召回或阅读全文。保留一个任务，因为发现、消歧、读附件必须在同一库归属和真实桌面流程中共同验收。
