# How to ask for Zotero work in Claude Code or Codex

This page covers one thing only: **how to ask in plain language**.

If `zot-skills` is installed, do not start from commands. Start from the task.

## The most common ways to ask

### 1. Find items

Just say:

- “Find papers in my Zotero library about reward hacking.”
- “Look up the paper with citekey Smith2024.”
- “Show me what is inside the Transformers collection.”

Helpful extra constraints:

- only items from a tag / creator / year
- start with the 3 most relevant
- after finding them, continue with citations

### 2. Pull evidence

Just say:

- “Pull the PDF annotations and notes for this paper.”
- “Show me the details, child items, and citation for this item.”
- “Download attachment ATCH005 into the current directory.”

Helpful extra constraints:

- “If PDF or annotation support is missing, tell me directly.”
- “If the key is wrong, point that out first.”

### 3. Build a workspace

Just say:

- “Create an llm-safety workspace and organize the relevant papers there.”
- “I want a long-lived workspace for mechanistic interpretability, with later Q&A.”

Helpful extra constraints:

- what the name should be
- whether import should be based on a query, collection, or tag
- whether indexing should happen right away

### 4. Save a search

Just say:

- “Save this filter as a Zotero saved search.”
- “List my current saved searches.”

Helpful extra constraints:

- what the search should be called
- what exact conditions should be stored
- whether you want to create or delete one

### 5. Mutate the library safely

Just say:

- “Add a note to this item.”
- “Tag it as priority.”
- “Put this item into that collection.”

It helps a lot to add:

- “If write access is missing, tell me what is missing first.”

That tells the agent to pass the safety gate instead of pretending the write succeeded.

### 6. Inspect config and troubleshoot

Just say:

- “Show me the current config and default profile first.”
- “If this environment is not ready yet, initialize it.”
- “Switch to the work profile before doing the rest.”

## The phrasing that makes things easiest

These sentence patterns work well:

- “In my Zotero library, …”
- “Start from the papers I already have in Zotero …”
- “If the current environment does not support this, tell me directly.”
- “After you find it, continue with …”
- “Do read-only analysis first. Don’t write to the library yet.”

These make it easier for the agent to decide:

- that the source of truth is Zotero
- whether the task is read-only or mutating
- whether doctor should run first
- what the next step should be after the first result

## What not to lead with

Do not start with:

- “Give me the command”
- “Explain how the skill calls the CLI”
- “List the internal runtime commands first”

That pulls the conversation back to the execution layer and slows down the actual task.

## A full example

You can say:

> Find papers in my Zotero library about reward hacking. Start with the 5 most relevant ones. Then pick the single best paper, show me its details, PDF annotations, and an APA citation. If the current environment cannot extract PDF evidence, tell me exactly what is missing.

That is better than splitting the task into four command-shaped requests. The agent can keep the whole Zotero workflow intact.
