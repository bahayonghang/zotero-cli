# item command

`item` handles single-item reads, exports, citations, attachments, and write actions.

## Read-oriented subcommands

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

Supported citation styles in the CLI:

- `apa`
- `nature`
- `vancouver`

## Write-oriented subcommands

```bash
zot --json item create --doi 10.1038/s41586-023-06139-9
zot --json item create --url https://arxiv.org/abs/2301.00001
zot --json item create --pdf paper.pdf
zot --json item update ATTN001 --title "New Title" --field publicationTitle=Nature
zot --json item trash ATTN001
zot --json item restore ATTN001
zot --json item attach ATTN001 --file supplement.pdf
```

These commands mutate the library, so check first that:

1. `doctor` succeeds
2. `ZOT_API_KEY` is configured
3. `ZOT_LIBRARY_ID` is configured

## note and tag subcommands

```bash
zot --json item note list ATTN001
zot --json item note add ATTN001 --content "Key finding: ..."
zot --json item note update NOTE001 --content "Revised note"

zot --json item tag list ATTN001
zot --json item tag add ATTN001 --tag important --tag reading-list
zot --json item tag remove ATTN001 --tag obsolete
```

## Usage guidance

- Use `library search` first to find items
- Use `workspace` when managing a group of items
- Review [Skills Safety](/en/skills/safety) or [Troubleshooting](/en/cli/troubleshooting) before mutating the library
