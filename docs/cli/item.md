# item 命令

`item` 负责单条目的读取、导出、PDF 处理，以及大部分会改库的动作。

## 读取类子命令

```bash
zot --json item get ATTN001
zot --json item related ATTN001 --limit 10
zot item open ATTN001
zot item open ATTN001 --url
zot --json item pdf ATTN001
zot --json item pdf ATTN001 --pages 1-3
zot --json item fulltext ATTN001
zot --json item children ATTN001
zot --json item download ATCH005
zot --json item deleted --limit 20
zot --json item versions --since 1200
zot --json item outline ATTN001
zot item export ATTN001 --format bibtex
zot item cite ATTN001 --style nature
```

说明：

- `item pdf` / `item fulltext` 当前都走 PDF 文本提取路径
- `item pdf --annotations` 用于读取 PDF 内已有批注
- `item children` 会批量返回 notes、attachments、annotations
- `item download` 需要 attachment key，不是父条目 key
- `item deleted` 用于看当前 Trash 里的条目
- `item versions` 返回远端 item version map，适合同步或排障
- `item outline` 依赖本地 PDF 可读且文档本身带有书签结构

支持的 citation style：

- `apa`
- `nature`
- `vancouver`

## 新增条目

显式别名：

```bash
zot --json item add-doi 10.1038/nature12373 --collection COLL001 --tag reading --attach-mode auto
zot --json item add-url https://arxiv.org/abs/1706.03762 --tag transformers --attach-mode auto
zot --json item add-file paper.pdf --doi 10.1038/nature12373 --collection COLL001 --tag imported
```

兼容旧调用：

```bash
zot --json item create --doi 10.1038/nature12373 --tag reading --attach-mode auto
zot --json item create --url https://example.com/paper --collection COLL001
zot --json item create --pdf paper.pdf --doi 10.1038/nature12373
```

`attach-mode`：

- `auto`
- `linked-url`
- `none`

`auto` 的 OA PDF cascade 顺序：

1. Unpaywall
2. arXiv relation
3. Semantic Scholar
4. PubMed Central

## 更新、回收站与附件

```bash
zot --json item update ATTN001 --title "New Title" --field publicationTitle=Nature
zot --json item trash ATTN001
zot --json item restore ATTN001
zot --json item attach ATTN001 --file supplement.pdf
zot --json item download ATCH005 --output downloads/
```

这些命令会改库。执行前应先确认：

1. `doctor` 已通过
2. 已配置 `ZOT_API_KEY`
3. 已配置 `ZOT_LIBRARY_ID`

注意：

- `item attach` 是上传新附件
- `item download` 是下载已有附件

## note / tag / annotation / scite

### notes

```bash
zot --json item note list ATTN001
zot --json item note search transformer --limit 10
zot --json item note add ATTN001 --content "Key finding: ..."
zot --json item note update NOTE001 --content "Revised note"
zot --json item note delete NOTE001
```

### tags

```bash
zot --json item tag list ATTN001
zot --json item tag add ATTN001 --tag important --tag reading-list
zot --json item tag remove ATTN001 --tag obsolete
zot --json item tag batch --tag test --add-tag verified --limit 50
```

### annotations

```bash
zot --json item annotation list --item-key ATTN001 --limit 50
zot --json item annotation search "core finding" --limit 20
zot --json item annotation create ATCH005 --page 1 --text "attention mechanisms" --color "#2ea043"
zot --json item annotation create-area ATCH005 --page 1 --x 0.10 --y 0.20 --width 0.30 --height 0.10
```

说明：

- annotation 创建首期只支持本地可读的 PDF attachment
- `create` 用 phrase 定位文本
- `create-area` 用归一化坐标创建区域批注

### Scite

```bash
zot --json item scite report --item-key ATTN001
zot --json item scite report --doi 10.1038/nature12373
zot --json item scite search "attention" --limit 10
zot --json item scite retractions --collection COLL001 --limit 50
```

## 使用建议

- 先用 `library search` 或 `library citekey` 找条目
- 单篇深入阅读时再转到 `item`
- 批量整理 collection 用 `collection`
- 长期主题集合用 `workspace`
