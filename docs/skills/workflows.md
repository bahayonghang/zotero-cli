# 典型工作流

## A：找论文并给出引用

目标：先检索，再读取单条目，最后输出引用。

```bash
zot --json library search "reward hacking" --limit 5
zot --json item get ATTN001
zot item cite ATTN001 --style apa
```

## B：建立长期使用的主题工作区

目标：围绕一组论文构建持续可用的查询空间。

```bash
zot --json workspace new mechinterp --description "Mechanistic interpretability papers"
zot --json workspace import mechinterp --search "mechanistic interpretability"
zot --json workspace index mechinterp
zot --json workspace query mechinterp "What methods are used to identify circuits?" --limit 5
```

## C：直接修改 Zotero

目标：写入标签、笔记、collection 关系或状态更新。

```bash
zot --json doctor
zot --json item tag add ATTN001 --tag priority
zot --json collection add-item COLL001 ATTN001
```

## 回归验证

仓库里已有 `skills/zot-skills/test-prompts.json`，可以用来检查 skill 是否仍按预期路由与执行。
