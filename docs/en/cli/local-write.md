# Local content operations (Zotero 10+)

Development-branch acceptance status: local tags, collection creation, and `library organize` passed automated checks and isolated profile/data acceptance on Windows with Zotero 10.0.7, including authorization, core writes, UI inspection, and readback after a normal restart. Group libraries, other operating systems, and interruption during a real submission remain untested; injected failures are automated evidence.

`item update`, collection membership, single-item `item tag add/remove`, `collection create`, and single-key `collection delete` support the official Local API. Keep Zotero running and enable local communication in Advanced settings. No Web API key or extra plugin is required. Zotero asks for authorization when a confirmed write needs it.

Run `zot --json doctor` first. `local_api_write.supported` describes protocol support; `credential_present` only describes a cached authorization. Doctor never prompts or attempts a write, and reports verified=false.

## Preview and confirm

Replace these example eight-character item/collection keys with real keys:

```bash
zot --json item update ITEMABCD --write-backend local --field DOI=10.1000/example
zot --json item tag add ITEMABCD --tag 待读 --tag "C++/安全" --write-backend local
zot --json item tag remove ITEMABCD --tag 过期 --write-backend local
zot --json collection create "待读" --write-backend local
zot --json collection create "子类" --parent ROOT2345 --write-backend local
zot --json collection add-item COLLB234 ITEMABCD --write-backend local
zot --json collection remove-item COLLA234 ITEMABCD --write-backend local
zot --json collection move-item ITEMABCD --from COLLA234 --to COLLB234 --write-backend local
zot --json collection delete COLL0001 --write-backend local
```

These return preview, or unchanged when no change is needed. Inspect before/after/skipped_fields, then confirm with the same arguments. Item and membership operations add `--confirm --expected-server-id <server_id> --expected-version <expected_version>`. `collection create` uses `--expected-library-version` instead. Never substitute Web/SQLite versions. Preview again on a conflict.

A move removes the source membership and adds the destination, preserving all other memberships and the item identity. It does not move files or copy items. Only top-level bibliographic items in the same library are supported. Targets come from arguments, not UI selection. move-item requires an explicit local backend.

## Single-item tags

Without `--write-backend local`, `item tag add/remove` still writes immediately through the Web API. `item tag batch` does not accept a local backend and is not a substitute for single-item local tags.

- A new tag is manual (`type` 0). An exact existing name is unchanged and keeps its original type, including automatic `type` 1.
- Remove deletes every association with that exact name. Same-name tags of different types are all removed, and the preview shows the complete before/after arrays.
- Untouched tag objects and their unknown properties are preserved. The conditional PATCH body contains only the complete `tags` array and the local version.
- Names are kept as entered, including CJK text, spaces, and symbols. There is no case, Unicode, or synonym folding, and nonempty names are not trimmed. Blank input is rejected.
- Readback compares tags as a multiset. Array order does not fail verification; omitted `type` and `0` are the same manual type, while `1` stays distinct.
- Only non-trashed top-level bibliographic items in the same library are eligible. `--field tags` cannot bypass this contract.

## Creating a collection

Without `--write-backend local`, `collection create` still uses the Web API. Rename and reparenting have no local write path. Single-key `collection delete` can explicitly use the local preview/confirm path.

```bash
zot --json collection create "待读" --write-backend local --confirm --expected-server-id <server_id> --expected-library-version <library_version>
```

Confirmation uses the library version, not an item version, and does not invent version 0 for an object that does not exist yet. Preview reads the complete collection list and the `Last-Modified-Version` from that response. An exact name under the same parent is rejected with the candidate keys; it is not reused, merged, or renamed. The same name is allowed under a different parent. Only a root collection, or one child under an existing same-library parent, can be created.

The POST body is a one-element array containing only name and parentCollection, with the Server ID and `If-Unmodified-Since-Version`. HTTP 200 is not success. applied requires an object-level success and a readback whose name, parent, and server match. Timeout, 5xx, or an unreliable response is unknown: there is no automatic resend and no later same-name object is claimed as this request. Creation does not change memberships, children, or attachments. The returned key can be passed to the existing `collection add-item` command.

## Deleting a collection

Without `--write-backend local`, `collection delete KEY` still deletes immediately through the Web API. An explicit local backend only previews: it does not read an authorization and does not send DELETE.

```bash
zot --json collection delete COLL0001 --write-backend local --confirm --expected-server-id <server_id> --expected-version <version> --expected-descendants-sha256 <sha256>
```

Preview lists the target and every descendant collection returned by the Local API collection list, and prints the SHA-256 of those keys sorted ascending. A `deleted` flag in that response is included in the list and the hash. Zotero 10.0.8 `GET /collections` omits trashed collections and ignores `includeTrashed`, so those keys are absent from the preview and the hash. This command does not open SQLite to fill the gap. Confirmation binds the collection object version, not the library version. Zero is valid only when that preview returned zero; do not invent it. A mismatch of Server ID, object version, or descendant hash is rejected before authorization or DELETE.

The delete permanently erases the collection and every descendant collection, including trashed descendants the list did not show. Children are not promoted. Items stay in the library and only lose those memberships. The command sends one DELETE for this key and does not call the bulk delete.

applied and verified require HTTP 204 from the same Server ID followed by GET 404. HTTP 200, a timeout, 5xx, or a mismatched Server ID is unknown; a later 404 must not be recorded as this delete. A collection that remains after 204 is verification-failed. DELETE 404 is absent: this call did not delete it. Those states are not success even when the envelope is ok, and they must not be blindly retried.

## Fields and creators

Fields are validated against the current itemType/schema. Empty fields are filled by default. Include `--overwrite` in both preview and confirmation to replace specified nonempty values. Clearing fields and changing itemType are outside this feature.

Use `--creators-file authors.json` for a complete ordered creator list:

```json
[
  {"creatorType":"author","firstName":"Ada","lastName":"Lovelace"},
  {"creatorType":"author","name":"Research Institute"}
]
```

Replacing existing creators requires --overwrite. Names are not split, appended, or deduplicated automatically. Native citationKey can be explicitly edited when supported by the schema; it is distinct from the item key. Better BibTeX regeneration is not guaranteed by this command.

## Results and boundaries

- applied + verified=true: the write response and readback both confirm the requested state.
- unknown: submission outcome is unknown. Even observed_expected_state=true cannot attribute the state to this request; do not blindly retry.
- verification-failed: write_accepted=true, but readback failed or differs.
- Conflicts, denied authorization, permissions, and invalid input use the standard error envelope.

Single-use authorization stays in memory. Only remembered authorizations are saved by profile and Server ID in local-api-credentials.json under the platform configuration directory (AppConfig::config_dir(); typically `%AppData%\zot` on Windows). Keys are excluded from config show and diagnostics.

Existing commands still default to Web. Local-only options are rejected on Web, and local failures never switch transports. Notes, attachments, `item tag batch`, collection rename/reparent, and online enrichment are not covered by the local writer. SQLite remains read-only.
