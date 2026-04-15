# Safety

## These actions are side-effectful by default

- `item create`
- `item add-doi`
- `item add-url`
- `item add-file`
- `item update`
- `item trash`
- `item restore`
- `item attach`
- `item note add`
- `item note update`
- `item note delete`
- `item tag add`
- `item tag remove`
- `item tag batch`
- `item annotation create`
- `item annotation create-area`
- `collection create`
- `collection rename`
- `collection delete`
- `collection add-item`
- `collection remove-item`
- `library duplicates-merge --confirm`
- `sync update-status --apply`

## Execution rules

1. If the user clearly asked for the action, do it
2. If the user only wants inspection or analysis, do not mutate the library
3. Confirm intent for these actions before proceeding:
   - `item trash`
   - `item note delete`
   - `collection delete`
   - `library duplicates-merge --confirm`
   - `sync update-status --apply`

## Read/write boundary

- Local reads: `zotero.sqlite`, attachment storage, and local index sidecars
- Remote writes: Zotero Web API

**Never write directly to `zotero.sqlite`.**

## What to do when write access is missing

If `doctor` shows missing credentials:

- stay in read-only mode
- tell the user exactly what is missing
- do not pretend the action succeeded

## Extra notes for annotations and attach mode

- annotation creation requires both local PDF access and write credentials
- `attach-mode auto` failing to find an OA PDF does not mean the whole command failed
