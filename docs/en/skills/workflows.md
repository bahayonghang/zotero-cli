# Workflows

## A: Find a paper and return a citation

Goal: search first, inspect one item, then return the citation.

```bash
zot --json library search "reward hacking" --limit 5
zot --json item get ATTN001
zot item cite ATTN001 --style apa
```

## B: Jump directly by citation key

Goal: locate a single item quickly when you already know the citekey.

```bash
zot --json doctor
zot --json library citekey Smith2024
zot item cite ATTN001 --style nature
```

## C: Build a library-level semantic index and search it

Goal: index the whole library or one collection, then run semantic or hybrid search.

```bash
zot --json doctor
zot --json library semantic-index --fulltext
zot --json library semantic-search "mechanistic interpretability" --mode hybrid --limit 5
```

## D: Inspect and create PDF annotations

Goal: confirm prerequisites, find the attachment, then create a highlight or area annotation.

```bash
zot --json doctor
zot --json item children ATTN001
zot --json item annotation list --item-key ATTN001
zot --json item annotation create ATCH005 --page 1 --text "attention mechanisms"
```

## E: Modify Zotero directly

Goal: write tags, notes, collection membership, or status updates.

```bash
zot --json doctor
zot --json item tag add ATTN001 --tag priority
zot --json collection add-item COLL001 ATTN001
```

## Regression prompts

The repository already includes `skills/zot-skills/test-prompts.json`, which can be used to validate whether the skill still routes and executes as intended.
