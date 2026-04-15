# Workflows

## A: Find a paper and return a citation

Goal: search first, inspect one item, then return the citation.

```bash
zot --json library search "reward hacking" --limit 5
zot --json item get ATTN001
zot item cite ATTN001 --style apa
```

## B: Build a persistent topic workspace

Goal: create a long-lived paper set for later query and retrieval.

```bash
zot --json workspace new mechinterp --description "Mechanistic interpretability papers"
zot --json workspace import mechinterp --search "mechanistic interpretability"
zot --json workspace index mechinterp
zot --json workspace query mechinterp "What methods are used to identify circuits?" --limit 5
```

## C: Modify Zotero directly

Goal: write tags, notes, collection membership, or status updates.

```bash
zot --json doctor
zot --json item tag add ATTN001 --tag priority
zot --json collection add-item COLL001 ATTN001
```

## Regression prompts

The repository already includes `skills/zot-skills/test-prompts.json`, which can be used to validate whether the skill still routes and executes as intended.
