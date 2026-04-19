# Migrating from `ref/zotagent`

This page answers three things:

1. what `ref/zotagent` already implements
2. how far the current Rust `zot` runtime matches it
3. how the remaining gaps should be filled

## Summary first

| Type | Conclusion |
| --- | --- |
| Covered | DOI / URL / file import, citation-key entry, single-PDF extraction, library semantic index/search |
| Partial substitute | local attachment indexing, full-text retrieval, index status inspection |
| Explicitly missing | `s2`, `add --s2-paper-id`, `search-in`, `metadata`, `read`, `expand`, zotagent-style `status`, zotagent-style `sync`, one-shot basic-field item creation |
| Extra capability in this repo | notes, tags, collections, saved searches, feeds, annotation creation, workspaces, Scite, duplicate merge, profile config, publication-status sync |

## What `zotagent` implements

From `ref/zotagent/README.md` and `ref/zotagent/src/cli.ts`, the current CLI focuses on four groups:

1. `add`
   - import by DOI
   - create an item from basic fields
   - search via `s2` and import by `paperId`
2. `sync` / `status`
   - scan Zotero attachments
   - extract and index PDF / EPUB / HTML / TXT
   - report local index status and paths
3. `search`
   - FTS5 full-text keyword search by default
   - vector search with `--semantic`
   - document-scoped search with `search-in`
   - bibliography-field search with `metadata`
4. `read`
   - block-window reads with `read`
   - cleaned full text with `fulltext`
   - hit expansion with `expand`
   - all addressable by `itemKey` or `citationKey`

## Current `zot` comparison

| `zotagent` feature | Current `zot` | Status | Notes |
| --- | --- | --- | --- |
| DOI / URL import | `item add-doi` / `item add-url` | Covered | Also supports `--attach-mode` |
| File import | `item add-file` / `item create --pdf` | Covered | Can also enrich from `--doi` |
| citation-key entry | `library citekey` | Covered | Resolve citekey first, then continue with `item` workflows |
| Single-PDF full text / outline / annotation reads | `item pdf` / `item fulltext` / `item outline` | Covered | Focused on PDF attachments |
| Library-level vector search | `library semantic-index` / `library semantic-search` | Covered | Supports `bm25` / `semantic` / `hybrid` |
| Curated workspace indexing and Q&A | `workspace index` / `workspace query` | Covered | This is a surface `zotagent` does not have |
| One-shot basic-field item creation | none | Missing | Current flow is DOI / URL / PDF / file first, then update |
| `s2` search | none | Missing | No Semantic Scholar search command exists today |
| `add --s2-paper-id` | none | Missing | No direct paperId import exists today |
| Attachment-wide indexing `sync` | no equivalent command | Partial | `library semantic-index --fulltext` covers metadata + PDF only |
| Index `status` | `doctor` + `library semantic-status` | Partial | Not a single equivalent command; missing zotagent-style path and error reporting |
| Default keyword full-text search | no equivalent command | Partial | `library semantic-search --mode bm25` requires the semantic index first and is not the same FTS5 syntax |
| Document-scoped `search-in` | none | Missing | Current fallback is extract text first, then locate in the agent |
| Bibliography-field `metadata` search | none | Missing | `library search` has filters, but not field-scoped metadata search |
| Block-window `read` | none | Missing | No block model or block-window read API exists |
| Context `expand` | none | Missing | No block-based expansion surface exists |
| `citationKey`-driven `fulltext` / `expand` | none | Missing | Today you must resolve citekey before switching to item key |
| Multi-attachment merged logical document | none | Missing | Current reads are mostly centered on the first PDF attachment |

## The biggest semantic mismatch

### 1. `sync` has the same name but a different meaning

- `zotagent sync`: attachment extraction + indexing
- `zot sync update-status`: publication-status checks for preprints

This should not be hidden behind an alias. In this repo, `sync` already has a stable meaning.

### 2. The existing retrieval base is `RagIndex`, not the zotagent document-manifest surface

This repo already has:

- `library semantic-index --fulltext`
- `library semantic-search --mode bm25|semantic|hybrid`
- `workspace index` / `workspace query`

But that stack is mainly built for:

- metadata chunks
- PDF chunks
- RAG and semantic retrieval

It is still not:

- a document-scoped block read surface
- `search-in`
- `expand`
- bibliography metadata search
- a unified extractor for PDF / EPUB / HTML / TXT attachments

### 3. The Rust CLI already has many Zotero-native workflows beyond zotagent

This repo already adds:

- notes / tags / collections read-write flows
- saved search
- feeds
- annotation creation
- duplicates / merge
- workspace
- Scite
- profile config
- publication-status sync

So the right completion path is not to clone zotagent command names. It is to fit the missing capability into the existing `library` / `item` / `workspace` structure.

## Completion plan

### Phase 1: extend the indexing foundation

Goal:

- turn the current PDF-leaning RAG base into a stable attachment text index

Main work:

- add a unified extractor abstraction in `zot-local` for PDF / EPUB / HTML / TXT
- persist manifests, extraction status, and error catalog state
- let multi-attachment items form one logical document
- add a read-only status API that reports index paths, attachment counts, error counts, and last update time

Constraints:

- do not change the meaning of `sync update-status`
- keep persistent index files out of temp directories

Verification:

- fixture coverage for PDF / EPUB / HTML / TXT
- re-runs skip unchanged files cleanly
- status output is stable and testable

### Phase 2: fill the read and retrieval surfaces

Goal:

- give agents an honest full-text read / expansion / document-search entrypoint

Main work:

- add a document-scoped search command equivalent to `search-in`
- add a block-window read command equivalent to `read`
- add a block-context expansion command equivalent to `expand`
- add field-scoped metadata search
- add `citationKey` selectors to text-reading commands

Naming rule:

- keep the `library` / `item` split
- do not restore flat top-level aliases

Verification:

- integration coverage for hit blocks, radius expansion, and citation-key routing
- multi-attachment items must keep stable block numbering

### Phase 3: fill the import and Semantic Scholar gaps

Goal:

- close the two largest holes inside the current `zotagent add` story

Main work:

- add a Semantic Scholar search surface
- add direct import by `paperId`
- add one-shot item creation from basic metadata fields

Constraints:

- continue writing through the Zotero Web API only
- keep the current JSON envelope

Verification:

- DOI, paperId, and manual metadata import flows each need regression coverage
- missing credentials must fail clearly instead of half-succeeding

### Phase 4: close the loop in skills, docs, and regression fixtures

Goal:

- make sure agents and docs only promise real capability

Main work:

- update `skills/zot-skills/SKILL.md`
- add ref\zotagent migration prompts and evals
- sync the bilingual docs

Verification:

- docs site build succeeds
- skill regression prompts cover the `search-in`, `status`, and `s2` gaps

## Current agent boundary

Until those phases land, the correct behavior is:

- for `search-in` / `expand`: state that the CLI has no native equivalent, then fall back to `item fulltext` / `item pdf`
- for `status`: use `doctor` + `library semantic-status`
- for `s2` / `paperId` import: say plainly that it is not implemented yet
- for zotagent-style `sync`: explain that only `library semantic-index --fulltext` or `workspace index` exists as a partial substitute

The rule is simple: do not document planned capability as if it already ships.
