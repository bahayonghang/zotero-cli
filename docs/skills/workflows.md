# 典型工作流

## A：找论文并给出引用

目标：先搜索，再读取单条目，最后输出引用。

```bash
zot --json library search "reward hacking" --limit 5
zot --json item get ATTN001
zot item cite ATTN001 --style apa
```

## B：按 citation key 直接定位

目标：已知 citekey，快速定位单篇条目。

```bash
zot --json doctor
zot --json library citekey Smith2024
zot item cite ATTN001 --style nature
```

## C：建立库级 semantic index 并检索

目标：对整个库或某个 collection 建索引，然后做 semantic / hybrid search。

```bash
zot --json doctor
zot --json library semantic-index --fulltext
zot --json library semantic-search "mechanistic interpretability" --mode hybrid --limit 5
```

## D：查看并创建 PDF annotation

目标：先确认附件和前置条件，再创建高亮或区域批注。

```bash
zot --json doctor
zot --json item children ATTN001
zot --json item annotation list --item-key ATTN001
zot --json item annotation create ATCH005 --page 1 --text "attention mechanisms"
```

## E：直接修改 Zotero

目标：写入 tags、notes、collection 关系或状态更新。

```bash
zot --json doctor
zot --json item tag add ATTN001 --tag priority
zot --json collection add-item COLL001 ATTN001
```

## 回归验证

仓库里已有 `skills/zot-skills/test-prompts.json`，可以用来检查 skill 是否仍按预期路由与执行。
