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

1. In a new environment or before any write, run `zot --json doctor` and inspect the four capabilities plus `selected_write_backend`
2. If the user only wants inspection or analysis, do not mutate the library
3. If the user names desktop or Web, use only that backend; otherwise use the effective profile. Keep failures on the selected backend and never fall back automatically
4. Confirm intent for these actions before proceeding:
   - `item trash`
   - `item note delete`
   - `item merge --confirm`
   - `collection delete`
   - `library saved-search delete`
   - `library duplicates-merge --confirm`
   - `library dedupe --confirm`
   - `sync update-status --apply`
5. Preview merge/dedupe first, report keeper, sources, backend, confidence, and skipped groups, then wait for confirmation
6. `library dedupe --confirm` skips low-confidence groups by default. Ordinary confirmation is not risk authorization; add `--include-low-confidence` only after showing those groups separately and obtaining explicit authorization

## Read/write boundary

- Local reads: `zotero.sqlite`, Zotero Local HTTP, attachment storage, and local index sidecars
- Desktop writes: the paired plugin currently supports only `item merge`, `library duplicates-merge`, and `library dedupe`
- Web writes: note, tag, collection, import, annotation, saved-search, status-sync, and explicitly selected Web merge/dedupe

**Never write directly to `zotero.sqlite`, and never describe Local HTTP as a write transport.**

## What to do when write access is missing

If `doctor` shows that the selected capability is unavailable:

- stay in read-only mode
- for desktop, distinguish Zotero stopped, plugin missing, unpaired, auth/protocol, and profile-mismatch states and give recovery for that backend
- for Web, identify missing `ZOT_LIBRARY_ID` / `ZOT_API_KEY`
- do not pretend the action succeeded
- do not switch backends on the user's behalf

If the task is configuration troubleshooting:

- inspect with `config show` first
- use `config init` or `config set` only when the user wants the environment changed
- treat profile switching as a side effect too

## Bridge installation and secret boundary

- `bridge setup` only generates the XPI and opens its folder; it does not install it or modify a Zotero profile
- the pairing code expires after five minutes, is single-use, and is shown only by Zotero UI
- never record a real code, desktop token, API key, or raw plan token in logs, prompts, issues, fixtures, or docs
- `bridge revoke` removes the current authorization; upgrading the plugin in the same profile preserves connection identity

## Extra notes for annotations and attach mode

- annotation creation requires both local PDF access and write credentials
- `attach-mode auto` failing to find an OA PDF does not mean the whole command failed
