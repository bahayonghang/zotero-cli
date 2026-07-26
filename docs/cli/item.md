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

这些命令会改库，当前走 Zotero Web API。执行前应先确认：

1. `doctor` 已通过
2. 已配置 `ZOT_API_KEY`
3. 已配置 `ZOT_LIBRARY_ID`

注意：

- `item attach` 是上传新附件
- `item download` 是下载已有附件

## merge

```bash
zot --json item merge KEEP001 DUPE001
zot --json item merge KEEP001 DUPE001 --confirm
```

说明：

- 默认先 preview，不加 `--confirm` 不落库
- preview 与 confirm 统一使用 Zotero Web API；确认前必须配置 `library_id` 与 `api_key`
- `--keep` 用来指定哪一条留下；不传时默认保留第一个 key
- 只支持 top-level bibliographic item
- preview 会列出 metadata 补齐、tags / collections 新增、child re-parent 数、重复 attachment 跳过数，以及 `skipped_incompatible_fields` 和 `relations_to_add`
- 不同 item type 的条目可以合并；keeper 保持自身类型，对该类型非法的源字段会被跳过并列进 `skipped_incompatible_fields`
- `--confirm` 后 keeper 会得到指向每个被并条目的 `dc:replaces` relation，Word / LibreOffice 里已插入的引文不会断链；被并条目进 Trash，不做永久删除
- Web API 合并保持多请求、非事务写入语义；`already_applied` 仍会随 applied 结果输出
- 如果你是先从重复检测结果里合并，也可以继续走 `library duplicates-merge`

## note / tag / annotation / scite

note、tag 和 annotation mutation 当前仍走 Web API；不要把 Zotero Local HTTP 或内置 connector 描述成这些命令的写通道。

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
# 先 preview；此命令不会写库
zot --json item tag batch --tag test --add-tag verified --limit 50
# 核对 matched/affected/sample_keys 后，用相同参数确认写入
zot --json item tag batch --tag test --add-tag verified --limit 50 --max-affected 50 --confirm
```

`item tag batch` 默认返回 `state: preview`，并区分过滤器总命中数 `matched` 与本次由
`--limit` 选中的 `affected`。只有 `--confirm` 才会调用 Web API；选中数超过
`--max-affected`（默认 50）时会在写入前拒绝。确认执行会返回逐项 add/remove 结果；
`state: partial` 或 `failed_operations > 0` 表示存在部分失败，不能当作全部成功。

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
