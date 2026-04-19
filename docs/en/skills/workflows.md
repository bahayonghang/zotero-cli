# Workflows

This page is organized by the kinds of requests users make in an agent session.

## A: Find a relevant set of papers first

User says:

> Find papers in my Zotero library about reward hacking. Start with the 3 most useful ones.

The agent should:

1. treat this as Zotero-first candidate retrieval
2. return relevant items instead of starting with commands
3. explain why those items deserve deeper reading

The reply should focus on:

- titles, authors, years
- why each item matched
- whether the next step should be detail, citation, or workspace creation

## B: Read the evidence behind one paper

User says:

> Pull the PDF annotations, notes, and child items for this paper.

The agent should:

1. check whether PDF and annotation support is available
2. gather metadata, attachments, annotations, and notes
3. say clearly what evidence is available and what is missing

The reply should focus on:

- the item's metadata
- available attachments and child items
- annotation and note content
- missing PDF/backend capability if relevant

## C: Build a long-lived workspace

User says:

> Create an llm-safety workspace, import the relevant papers, and make it ready for later Q&A.

The agent should:

1. recognize this as a long-lived workspace task, not a one-off search
2. choose or normalize a kebab-case workspace name
3. keep import, indexing, and query readiness as separate steps

The reply should focus on:

- the workspace name
- what will be imported
- whether indexing is ready
- what can happen next

## D: Save a search

User says:

> Save this filter as a Zotero saved search so I can reuse it later.

The agent should:

1. treat the task as storing search conditions
2. state what conditions will be saved
3. clarify that a saved search is not a static result snapshot

The reply should focus on:

- the saved-search name
- the stored conditions
- how it can be reused later

## E: Download an attachment

User says:

> Download attachment ATCH005 into the current directory.

The agent should:

1. recognize that this requires an attachment key
2. ask only if the user provided a parent item key instead
3. return the actual file path after download

The reply should focus on:

- which attachment was downloaded
- where it was saved
- whether a missing file or wrong key caused failure

## F: Mutate Zotero safely

User says:

> Add a note to this item and tag it as priority.

The agent should:

1. recognize it as a write request
2. check doctor or write prerequisites first
3. state the intended mutation before executing it

The reply should focus on:

- what changed
- whether there are side effects
- what is missing if write access is unavailable

## G: Inspect configuration first

User says:

> I’m about to do Zotero work in Codex. Show me the current config and default profile first.

The agent should:

1. inspect config and profiles before falling back to raw env-var advice
2. surface missing configuration explicitly
3. move to doctor only if needed

The reply should focus on:

- default and selected profile state
- effective config
- missing values
- whether the next step is config init, config set, or the actual Zotero task

## H: List the most recently added items first

User says:

> Show me the last 10 items added to the library.

The agent should:

1. recognize that this is not keyword search
2. route to the recent-N path instead of `library search`
3. use the returned items as the handoff point for later item or workspace work

The reply should focus on:

- which items were added most recently
- titles, authors, years
- whether the next step should be detail, workspace, or further filtering

## I: Preview a merge before applying it

User says:

> Preview what would change if KEEP001 and DUPE001 were merged. Only apply it after I confirm.

The agent should:

1. recognize this as manual merge, not just duplicate inspection
2. return the preview first
3. perform the write only after explicit confirmation

The reply should focus on:

- which metadata fields would be filled
- which tags and collections would be added
- how many child items would be re-parented
- how many duplicate attachments would be skipped

## Regression coverage

The repo already includes:

- `skills/zot-skills/test-prompts.json`
- `skills/zot-skills/evals/evals.json`

They cover search, evidence extraction, workspace setup, saved searches, recent-N routing, manual merge preview, attachment download, and config inspection.
