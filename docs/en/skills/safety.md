# Safety

## These actions are side-effectful by default

- `item create`
- `item update`
- `item trash`
- `item restore`
- `item attach`
- `item note add/update`
- `item tag add/remove`
- `collection create/rename/delete/add-item/remove-item`
- `sync update-status --apply`

## Execution rules

1. If the user clearly asked for the action, do it
2. If the user only asked to inspect or analyze, do not mutate the library
3. Confirm intent for destructive actions:
   - `item trash`
   - `collection delete`
   - `sync update-status --apply`

## Read/write boundary

- Local reads: `zotero.sqlite` and attachment storage
- Remote writes: Zotero Web API

**Never write directly to `zotero.sqlite`.**

## What to do when write access is missing

If `doctor` shows missing credentials:

- stay in read-only mode
- tell the user what is missing
- do not pretend the action succeeded
