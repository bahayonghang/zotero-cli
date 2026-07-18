# Implement — optimize zot skill for connector-based local access

前置:两个前置子任务已合入。先跑 `zot --json doctor` 与 `zot item import --help`
(或 `cargo run -q -p zot-cli -- ...`)拿到真实命令面与 flag 名,再改文档,避免写出
不存在的命令。

## Checklist

1. [ ] 基线快照:记录当前 CLI 命令面
   - `zot --help`、`zot item --help`、`zot item import --help`、`zot --json doctor`
     → 作为 SKILL.md/evals 核对的事实来源。
2. [ ] `skills/zot/SKILL.md` frontmatter `description`:删「desktop bridge setup/pair/status」,
       加「connector 本机导入 BibTeX/RIS」。保持描述其余边界不变。
       → verify: `grep -n "bridge\|pair\|write-backend\|desktop_write" skills/zot/SKILL.md` 为空
3. [ ] SKILL.md 正文:改写写入后端表(三→两行)、写入决策顺序、安全门、fallback、
       硬约束、典型映射;新增「导入文献」意图桶;输出契约补双 key 区别 + 失败点名门;
       加 cite-into-draft 映射示例。import 一律 `--confirm`。
       → verify: 同上 grep 为空;逐条对照步骤 1 快照
4. [ ] `skills/zot/evals/evals.json`:重写 id 27-31、34-35(删 desktop/bridge 断言),
       新增 import / 双 key / merge-需要-Web 场景;保持 35 条且 id 与 test-prompts 对齐。
       → verify: `python -c "import json;d=json.load(open('skills/zot/evals/evals.json',encoding='utf-8'));print(len(d['evals']))"` == test-prompts 条数;
       grep evals.json 无 bridge/desktop_write
5. [ ] `skills/zot/test-prompts.json`:同步改写对应 id 的 prompt/expected;删 bridge 触发,
       加 import / 双 key / merge-需要-Web。
       → verify: 两份 id 集合一致;grep 无 bridge
6. [ ] 触发核对(人工 + fixture,无 trigger_eval.py):确认新 description 不误触发
       near-neighbor(通用找论文 / 引用格式教学 / 非 Zotero PDF),import 场景能触发;
       结论落进 evals/test-prompts 正反例。
7. [ ] 重生成镜像:`just install`(把 skills/ 拷进 .agents/skills 和 .claude/skills)。
       → verify: `just skills-check` 通过(三处内容哈希一致)
8. [ ] 全量门。
       → verify: `just ci` 全绿(version-check / fmt / check / clippy / test / skills-check)

## Review gate

- SKILL.md 描述的每个命令都能在步骤 1 快照里找到;没有已删除(bridge)或未实现的命令。
- import 相关全部用 `--confirm`,零 `--yes`。
- evals.json 与 test-prompts.json 的 id 集合严格一致(便于交叉引用)。

## Rollback

- 纯文档/fixture 改写;revert 本任务提交即可,同时 `just install` 复位镜像。
