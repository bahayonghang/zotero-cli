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
- `item merge --confirm`
- `collection create`
- `collection rename`
- `collection delete`
- `collection add-item`
- `collection remove-item`
- `library saved-search create`
- `library saved-search delete`
- `library duplicates-merge --confirm`
- `library dedupe --confirm`
- `sync update-status --apply`
- `config init`
- `config set`
- `config profiles use`

## Execution rules

1. In a new environment or before any write, run `zot --json doctor` and inspect the four capabilities
2. If the user only wants inspection or analysis, do not mutate the library
3. Only new BibTeX/RIS imports can use the connector; merge/dedupe and all other mutations use the Web API
4. Confirm intent for these actions before proceeding:
   - `item trash`
   - `item note delete`
   - `item merge --confirm`
   - `collection delete`
   - `library saved-search delete`
   - `library duplicates-merge --confirm`
   - `library dedupe --confirm`
   - `sync update-status --apply`
5. Preview merge/dedupe first, report keeper, sources, confidence, and skipped groups, then wait for confirmation
6. `library dedupe --confirm` skips low-confidence groups by default. Ordinary confirmation is not risk authorization; add `--include-low-confidence` only after showing those groups separately and obtaining explicit authorization

## Read/write boundary

- Local reads: `zotero.sqlite`, Zotero Local HTTP, attachment storage, and local index sidecars
- Connector writes: only new BibTeX/RIS imports, with the selected Zotero UI target rechecked for writability immediately before confirmation
- Web writes: merge/dedupe, note, tag, collection, Web import, annotation, saved-search, and status-sync

**Never write directly to `zotero.sqlite`, and never describe Local HTTP as a write transport.**

## What to do when write access is missing

If `doctor` shows that the required capability is unavailable:

- stay in read-only mode
- for connector import, start Zotero and select a writable library or collection in its UI
- for Web, identify missing `ZOT_LIBRARY_ID` / `ZOT_API_KEY`
- do not pretend the action succeeded

If the task is configuration troubleshooting:

- inspect with `config show` first
- use `config init` or `config set` only when the user wants the environment changed
- treat profile switching as a side effect too

## Connector target boundary

- the connector needs no plugin, pairing code, or token
- both dry-run and confirm inspect the current target; confirm rechecks `editable` / `libraryEditable` immediately before import
- a read-only target must fail closed before any import request is sent
- never record an API key in logs, prompts, issues, fixtures, or docs

## Extra notes for annotations and attach mode

- annotation creation requires both local PDF access and write credentials
- `attach-mode auto` failing to find an OA PDF does not mean the whole command failed
