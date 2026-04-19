# Routing

This page is about how the agent should interpret plain-language Zotero requests.

It is not a command catalog.

## Route by user intent

| What the user says | What the agent should infer | What the reply should focus on |
| --- | --- | --- |
| “Find papers in my Zotero library about reward hacking” | surface candidate items first | which items match and why |
| “Look up Smith2024” | direct citation-key lookup | metadata, citation, fallback state |
| “Pull the PDF annotations and notes for this paper” | evidence extraction for one item | metadata, children, annotations, missing capability |
| “Create an llm-safety workspace” | build a long-lived topic workspace | naming, import scope, index prerequisites |
| “Save this as a Zotero saved search” | store reusable search conditions | what conditions were saved |
| “Download attachment ATCH005” | local attachment download | which attachment, where it was saved |
| “Add a note to this item” | controlled mutation | what will change and whether write access exists |
| “Show me the current config and default profile” | configuration inspection | effective config, default profile, missing pieces |

## Quick rule of thumb

- one item or a few items: treat it as lookup or evidence extraction
- a long-lived topic set: treat it as a workspace request
- “save this filter” or “reuse this later”: treat it as a saved-search request
- “download the attachment”: treat it as an attachment task, not upload
- any mutation: go through the safety gate first
- any “why is this broken?” question: start with doctor or config

## When to run doctor first

Run `doctor` first when:

- this is a new environment
- the task will mutate the library
- the task depends on PDF / outline / annotation / attachment behavior
- the task needs semantic indexing or workspace query
- the task depends on citation-key lookup
- the user is troubleshooting configuration or profile state

## Division of labor between skills and CLI docs

- Skills pages answer: “How should I ask, and how will the agent interpret it?”
- CLI pages answer: “What command and flags exist underneath?”

Read the skills pages first. Use the CLI pages as reference.
