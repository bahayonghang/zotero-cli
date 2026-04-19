# item command

`item` handles single-item inspection, export, PDF-related work, and most library-mutating actions.

## Read-oriented subcommands

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

Notes:

- `item pdf` and `item fulltext` currently share the PDF text-extraction path
- `item pdf --annotations` reads annotations already embedded in the PDF
- `item children` batches notes, attachments, and annotations together
- `item download` requires an attachment key, not a parent item key
- `item deleted` lists items currently in Trash
- `item versions` returns the remote item-version map for sync or troubleshooting work
- `item outline` depends on local PDF availability and the document actually containing bookmarks

Supported citation styles:

- `apa`
- `nature`
- `vancouver`

## Creating items

Explicit aliases:

```bash
zot --json item add-doi 10.1038/nature12373 --collection COLL001 --tag reading --attach-mode auto
zot --json item add-url https://arxiv.org/abs/1706.03762 --tag transformers --attach-mode auto
zot --json item add-file paper.pdf --doi 10.1038/nature12373 --collection COLL001 --tag imported
```

Backwards-compatible `create` usage:

```bash
zot --json item create --doi 10.1038/nature12373 --tag reading --attach-mode auto
zot --json item create --url https://example.com/paper --collection COLL001
zot --json item create --pdf paper.pdf --doi 10.1038/nature12373
```

`attach-mode`:

- `auto`
- `linked-url`
- `none`

The `auto` OA PDF cascade runs in this order:

1. Unpaywall
2. arXiv relation
3. Semantic Scholar
4. PubMed Central

## Update, trash, restore, and attachments

```bash
zot --json item update ATTN001 --title "New Title" --field publicationTitle=Nature
zot --json item trash ATTN001
zot --json item restore ATTN001
zot --json item attach ATTN001 --file supplement.pdf
zot --json item download ATCH005 --output downloads/
```

These commands mutate the library. Check first that:

1. `doctor` has been run
2. `ZOT_API_KEY` is configured
3. `ZOT_LIBRARY_ID` is configured

Notes:

- `item attach` uploads a new attachment
- `item download` downloads an existing attachment

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

Notes:

- annotation creation is PDF-first and requires a locally readable PDF attachment
- `create` locates text by phrase
- `create-area` creates an image-style annotation from normalized coordinates

### Scite

```bash
zot --json item scite report --item-key ATTN001
zot --json item scite report --doi 10.1038/nature12373
zot --json item scite search "attention" --limit 10
zot --json item scite retractions --collection COLL001 --limit 50
```

## Usage guidance

- find the item first with `library search` or `library citekey`
- switch to `item` when the task becomes single-item and detail-oriented
- use `collection` for actual Zotero collection maintenance
- use `workspace` for long-lived topic sets
