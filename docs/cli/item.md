# item 命令

`item` 负责单条目的读取、导出、引用、附件与写操作。

## 读取类子命令

```bash
zot --json item get ATTN001
zot --json item related ATTN001 --limit 10
zot item open ATTN001
zot item open ATTN001 --url
zot --json item pdf ATTN001
zot --json item pdf ATTN001 --annotations
zot item export ATTN001 --format bibtex
zot item cite ATTN001 --style apa
```

支持的 citation style 来自 CLI：

- `apa`
- `nature`
- `vancouver`

## 写操作子命令

```bash
zot --json item create --doi 10.1038/s41586-023-06139-9
zot --json item create --url https://arxiv.org/abs/2301.00001
zot --json item create --pdf paper.pdf
zot --json item update ATTN001 --title "New Title" --field publicationTitle=Nature
zot --json item trash ATTN001
zot --json item restore ATTN001
zot --json item attach ATTN001 --file supplement.pdf
```

这些命令会改库，执行前应先确保：

1. `doctor` 已通过
2. 已配置 `ZOT_API_KEY`
3. 已配置 `ZOT_LIBRARY_ID`

## note 与 tag 子命令

```bash
zot --json item note list ATTN001
zot --json item note add ATTN001 --content "Key finding: ..."
zot --json item note update NOTE001 --content "Revised note"

zot --json item tag list ATTN001
zot --json item tag add ATTN001 --tag important --tag reading-list
zot --json item tag remove ATTN001 --tag obsolete
```

## 使用建议

- 要找条目，先用 `library search`
- 要整理一组条目，优先考虑 `workspace`
- 要改库，先看 [Skills 安全边界](/skills/safety) 或 [故障排查](/cli/troubleshooting)
