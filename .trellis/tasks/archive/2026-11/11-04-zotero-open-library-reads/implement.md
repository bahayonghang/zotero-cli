# 实施与验收顺序

1. 建立只读基线：对齐 item get 的 item/notes/attachments、children 的顺序与批量 key、父条目解析 PDF 的规则，盘点字段可表示性。用已提交且保持连接的 exclusive SQLite fixture 补充原有 BEGIN EXCLUSIVE 测试；测试不触及真实库。如果必须改变公开 JSON 或旧版兼容范围，先回到计划评审。
2. 实现 zot-desktop GET 和指定命令路由。模拟个人库、群组同 key、profile/数据目录不符、未绑定显式选择、版本变化、禁用、关闭、权限错误、HTTP 重定向和 404。对命中桌面路径的测试计数零次快照打开、零次授权/写请求；跨源失败不混读。
3. 接入 library search / citekey：先验证原生 citationKey、Extra、插件生成键、去重冲突与库归属；验证唯一题名、重名、部分题名、摘要/标签主题词、期刊字段与跨页干扰项。开放和关闭状态用相同 fixture 检查候选与解析结果，锁住源库时也能完整走到 PDF。单独断言本地 BBT 只允许 item.search，无任何写方法。
4. 接入 item get/children/pdf 的实际 clap 入口，保持现有输出和 SQLite 快照回归，核对 stored/linked、中文路径、多附件、文件缺失和 Pdfium 失败。用固定第一页文本验证真的读到了 PDF。覆盖桌面关闭时的旧路径。
5. 更新 doctor 分项能力、错误提示、skills/zot/SKILL.md 与 skills/zot/evals或test-prompts 对应用例；按 justfile 同步镜像并运行 just skills-check。覆盖“题名或 citation key 找单篇”“按主题或期刊找一批”“已知 key”“403”“关闭状态”，以及写请求须另走独立流程。
6. 运行 cargo test -p zot-local、cargo test -p zot-desktop、cargo test -p zot-cli 及 just ci。Windows 桌面验收使用可丢弃的测试库，预置带确定正文的 PDF：打开桌面且启用 API，执行 doctor，从题名或 citation key 定位同一条目并按主题/期刊取得已知集合，再执行 item get、item children、item pdf --pages 1，核对返回；关闭桌面，重复同一命令。记录各结果、请求/快照计数（测试仪表）与真实观察，不用耗时阈值推断来源。同步活动中不能证明同一版本的多请求结果时，有界重试或明确失败。

回退范围仅限新增的读路由、诊断与 skill。保留原有快照测试、写后端和安全门。用户已于 2026-11-04 批准按本顺序实施。实施阶段不提交，不创建远端 issue。
