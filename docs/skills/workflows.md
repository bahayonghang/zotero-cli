# 典型工作流

这一页按“用户在 agent 里怎么提任务”来组织。

## A：先找出一组相关论文

用户会说：

> 帮我在 Zotero 里找 reward hacking 相关的论文，先给我最值得看的 3 篇

agent 应该做的事：

1. 先把任务理解成 Zotero 库内候选条目检索
2. 返回最相关条目，而不是先贴命令
3. 说明为什么这些条目值得继续深读

回答重点：

- 条目标题、作者、年份
- 为什么匹配
- 下一步是看详情、引用，还是建 workspace

## B：深读一篇论文的证据面

用户会说：

> 把这篇论文的 PDF 批注、note、children 都拉出来

agent 应该做的事：

1. 先确认 PDF / annotation 能力是否可用
2. 再把 metadata、附件、批注、笔记整合回来
3. 明确哪些是现成证据，哪些当前取不到

回答重点：

- 这篇条目的 metadata
- 有哪些子项和附件
- 批注和 note 里的关键信息
- 当前环境缺不缺 PDF backend / 写权限

## C：围绕主题建长期工作面

用户会说：

> 给我建一个 llm-safety workspace，把相关论文都整理进去，后面我要做问答检索

agent 应该做的事：

1. 把任务理解成“长期主题工作区”，不是一次性搜索
2. 选一个合适的 kebab-case 名称
3. 说明导入、建索引、问答检索是三步，不要混成一句

回答重点：

- workspace 名
- 计划导入什么范围
- 是否已经具备索引前提
- 建好后能继续做什么

## D：保存查询条件

用户会说：

> 把这个筛选条件保存成一个 Zotero saved search，后面我要反复用

agent 应该做的事：

1. 识别这是“保存条件”，不是“现在跑一次搜索”
2. 明确保存的是什么条件
3. 提醒 saved search 不是结果快照

回答重点：

- 保存查询的名字
- 条件内容
- 它以后适合怎么复用

## E：下载附件

用户会说：

> 把附件 ATCH005 下载到当前目录

agent 应该做的事：

1. 识别下载面需要 attachment key
2. 如果用户给的是父条目 key，要先指出还缺 attachment key
3. 下载后返回实际文件路径

回答重点：

- 下载的是哪个附件
- 保存到了哪里
- 如果失败，是 key 不对还是本地文件缺失

## F：写入前先过安全门

用户会说：

> 给这篇文献加一条 note，再打上 priority 标签

agent 应该做的事：

1. 先把它识别成写操作
2. 检查 doctor / 写权限
3. 明确即将发生的变更，再执行

回答重点：

- 改了什么
- 有没有副作用
- 如果没权限，缺少什么

## G：配置排障

用户会说：

> 我在 Codex 里要开始做 Zotero 任务了，先帮我看看当前配置和默认 profile

agent 应该做的事：

1. 先看 config / profile，而不是直接让用户背环境变量
2. 如果配置缺失，明确指出缺哪个字段
3. 如果需要，再引导到 `doctor`

回答重点：

- 当前默认 profile
- 当前有效配置
- 缺失项
- 下一步是 `config init`、`config set` 还是直接继续任务

## H：先看最近入库的条目

用户会说：

> 给我看最近 10 条刚进 Zotero 库的文献

agent 应该做的事：

1. 识别这不是关键词搜索
2. 走 recent-N 路由，而不是 library search
3. 如果用户后面还要深读，再从返回结果里继续转 item / workspace

回答重点：

- 最近入库的是哪些条目
- 它们的标题、作者、年份
- 下一步是看详情、建 workspace，还是继续过滤

## I：先 preview 再 merge

用户会说：

> 先帮我预览合并 KEEP001 和 DUPE001 会改什么；我确认后再真的合并

agent 应该做的事：

1. 识别这是手工 merge，不是单纯 duplicate 检查
2. 先返回 preview
3. 只有用户明确确认，才执行 `--confirm`

回答重点：

- 会补哪些 metadata
- 会新增哪些 tags / collections
- 会 re-parent 多少 children
- 会跳过多少重复 attachment

## 回归验证

仓库里已有这些回归资产：

- `skills/zot-skills/test-prompts.json`
- `skills/zot-skills/evals/evals.json`

它们覆盖查条目、取证据、workspace、saved search、recent-N、手工 merge、附件下载、配置排障等场景。
