# Local API Content Operations

## 1. Scope / Trigger

Use when editing `zot-desktop/src/local_api.rs`, CLI `local_write.rs`,
`local_credentials.rs`, local command flags, or `doctor.local_api_write`.
Zotero 10+ official Local API writes are separate from connector imports,
SQLite reads, and the Web API. Ordinary open-library GETs are also separate:
see [Desktop Library Reads](./desktop-read.md). Do not send write
authorization on those reads, and do not route them through `local_library()`.
Evidence: task `10-13-zotero-local-content-operations`.

## 2. Signatures

```text
item update KEY --write-backend local [--title VALUE] [--date VALUE]
  [--field NAME=VALUE ...] [--creators-file FILE] [--overwrite]
item tag add KEY --tag NAME ... --write-backend local
item tag remove KEY --tag NAME ... --write-backend local
collection create NAME [--parent KEY] --write-backend local
library organize plan --file requests.json --output plan.json --write-backend local [--max-affected 50]
library organize apply --plan plan.json --confirm --expected-plan-sha256 HASH --report report.json --write-backend local [--max-affected 50]
collection add-item COLLECTION ITEM --write-backend local
collection remove-item COLLECTION ITEM --write-backend local
collection move-item ITEM --from SOURCE --to TARGET --write-backend local
collection delete KEY --write-backend local

Item and membership operations: [--confirm --expected-server-id ID --expected-version N]
Collection create: [--confirm --expected-server-id ID --expected-library-version N]
Collection delete: [--confirm --expected-server-id ID --expected-version N --expected-descendants-sha256 HASH]
GET /api/
GET /api/itemTypeFields?itemType=TYPE
GET /api/itemTypeCreatorTypes?itemType=TYPE
GET /api/{users/0|groups/ID}/collections
GET /api/{users/0|groups/ID}/{items|collections}/KEY
POST /api/local/authorize {"appName":"zot CLI"}
POST /api/{users/0|groups/ID}/collections
DELETE /api/{users/0|groups/ID}/collections/KEY
PATCH /api/{users/0|groups/ID}/items/KEY
```

PATCH includes `Zotero-Server-ID`, `Zotero-API-Key`,
`If-Unmodified-Since-Version`, and JSON `version` plus changed fields only.
A tag PATCH contains only `tags` and `version`; `tags` is the complete target
array, not a delta.
`LocalObject` validates outer/data key and version consistency. Keys are eight
uppercase ASCII letters/digits. Group IDs are public IDs from `LibraryScope`.

## 3. Contracts

- Production endpoint is fixed loopback HTTP port 23119. No proxies or redirects.
  Alternate origins exist only through the `test-support` constructor.
- Probe requires Zotero major >=10, API 3, and nonempty Server ID. Server identity
  is checked on every read and successful write. Local versions never come from
  Web or SQLite. Local commands never call `ctx.remote()`/`ctx.local_library()`.
- Existing update/add/remove, single-item tag add/remove, collection create,
  and single-key collection delete default to Web. Move is explicitly local
  only. Web rejects local confirmation, overwrite, creators-file,
  library-version, and descendant-hash flags. `item tag batch` has no local
  backend. Collection rename and reparent remain Web-only.
- Preview/no-op never reads credential contents, authorizes, or sends PATCH.
  Confirmation re-reads the item and rejects stale server/version before auth.
  Use the same library, key, mutations, and flags as the reviewed preview.
- Only non-trashed, top-level bibliographic items. Validate collection keys in
  the same library. Move is `(members - source) + target`, in one PATCH;
  preserve all other memberships. Repeated move to an existing target is no-op.
- Fields must be in the item's current schema and not reserved identity/array
  fields. Strings cannot be cleared. Fill empty/missing/whitespace-only values
  by default; nonempty differing values are skipped unless `--overwrite`.
  `tags` stays reserved for the tag commands; `--field tags` cannot bypass them.
- Creators are a nonempty complete ordered array, with schema-valid roles and
  either `name` or `firstName`/`lastName`. Do not concatenate, infer, or dedupe.
  Replacing a nonempty array requires overwrite. Notes/attachments and itemType
  conversion are excluded. Native `citationKey` is schema-dependent.
- Local collection create reads the complete collection list and the
  `Last-Modified-Version` from the same response. Omit the default limit; if a
  next link exists, follow only its `start` on the same endpoint and require
  the library version to stay unchanged. A missing library version, incomplete
  total, or version change is a protocol/conflict error, never an unconditional
  POST. Confirmation uses that library version, not an item version and not a
  synthetic 0. POST one object with only `name` and `parentCollection`, plus
  Server ID and `If-Unmodified-Since-Version`; do not send `Zotero-Write-Token`.
  HTTP 200 is not success. Index 0 must be `successful` and readback must match
  name, parent, and server. `failed` is a rejection. Timeout, 5xx, and
  malformed success responses are `unknown`, with no resend and no same-name
  claim. Same-parent exact names are rejected with candidate keys.
- Single-key local collection delete previews without credentials, authorization,
  DELETE, or SQLite. The key is eight uppercase letters or digits. The report
  lists every descendant collection returned by the Local API collection
  list, sorted by key, and keeps a deleted flag when that response includes
  one. Zotero 10.0.8 `GET /collections` omits trashed collections and ignores
  `includeTrashed`, so those keys are absent from the preview and the hash.
  Do not open SQLite to fill that gap, and do not describe the hash as a
  complete trash-inclusive set on that server. SHA-256 covers the listed keys
  joined by newline with no trailing newline. The empty set hashes the empty
  byte string. Confirmation requires the preview's Server ID, collection
  object version (zero only when that preview returned zero), and descendant
  hash. A library version is display-only. Server or version mismatch is
  `local-api-conflict`; hash mismatch is `local-delete-descendants`. Both
  refuse before auth and before DELETE.
  One `DELETE .../collections/KEY` sends the API key, Server ID, and
  `If-Unmodified-Since-Version`, with no body, query, or write token.
  Descendant collections are permanently erased with the target, including
  trashed descendants the list did not show. Items stay in the library and
  only lose those memberships. Children are not promoted.
  DELETE HTTP 204 with the same Server ID is accepted; `applied` requires the
  following GET to be 404. Do not treat this 204 as unknown the way collection
  create does. HTTP 200, timeout, 5xx, or a mismatched Server ID is `unknown`
  and must not be followed by a GET that reinterprets the outcome. DELETE 404
  is `absent`: this call did not delete the object. HTTP 412 is
  `local-api-conflict`, not unknown. There is no 50-key bulk delete.
- `library organize` is local-only. It does not replace `item tag batch` or create
  collections. Request v1 is a field whitelist. Planning reads the Local API only.
  SHA-256 covers the exact plan bytes and is not embedded in the file. Changed
  items count once, the default ceiling is 50, and plan/apply must use the same
  explicit ceiling. Each item gets one conditional PATCH. The first failed,
  conflict, unknown, or verification-failed item stops the rest as not-attempted.
  Exit 0 only when every item is applied or unchanged; other completed reports
  still use one JSON envelope and a non-zero exit. In-flight after a save failure
  is pending review, not success.
  Validate original requests, unique keys, summary counts, changed flags, exact
  field sets and collection identities; recompute after from reviewed before,
  then compare before/after/title against live data in preflight. A valid hash
  does not make a malformed plan executable. Recheck item versions even for
  no-ops, and collection identity/version before each submission and after auth.
  Item preconditions do not make collection reads and PATCH atomic.
  Refresh counts before every durable snapshot. Every persistence error stops
  execution and forces non-zero exit, including the final save. Preserve known
  stdout outcomes on save failure; only the stale disk in-flight record remains
  uncertain. Initial and remaining items are not-attempted; never claim a batch
  succeeded solely because every item succeeded when the report could not save.
- Single-item local tags: new values are manual (`type` 0). An exact existing
  name is a no-op and keeps its type, including automatic `type` 1. Remove
  drops every association with that exact name and preserves every other tag
  object, including unknown properties. Blank names are rejected; nonempty
  names are not trimmed or case/Unicode folded. Duplicate requested names are
  deduped. Illegal server `tags` are rejected, never replaced with `[]`.
  Readback is a multiset: order may differ, omitted type equals 0, and type 1
  stays distinct. Missing `tags` means no tags; a present non-array does not.
- Authorization occurs only for a confirmed change. `SecretString` prevents
  Debug leakage. Remembered keys are partitioned by effective profile + Server
  ID in `AppConfig::config_dir()/local-api-credentials.json`; no temp/CWD
  fallback. `write_secret_atomically` uses restrictive atomic persistence.
  One-time keys remain in process memory. Never log request keys/response bodies.
- A 401 can invalidate the cached key, authorize, and retry once. No retries
  after timeout, 5xx, or unreliable successful responses: report `unknown` and
  attempt readback for an item PATCH. Never switch backends. A PATCH 204 from
  the expected server is accepted; it becomes `applied` only after matching
  same-server readback. Collection create treats HTTP 204 as unknown. Collection
  delete does not send a verifying GET after an unknown DELETE.
- Report fields include `backend`, `state`, `server_id`, `library`, `item_key`,
  `expected_version`, `before`, `after`, `skipped_fields`, collection names,
  `write_accepted`, `verified`, `observed_expected_state`, and optional `error`.
  Tag operations use `tag-add` / `tag-remove`. Collection arrays compare as
  memberships; creator arrays compare in order; tag arrays compare as the
  multiset above.
- The existing JSON envelope stays at API version 1. A completed report uses
  `ok:true` even for `unknown` / `verification-failed`; callers must check state
  and verified. Pre-submission/rejection errors use `ok:false` and exit 1.
- Doctor only reports reachability/support/credential presence; `verified` is
  false. It never authorizes or test-writes. Secret cache decode errors are
  generic and never expose contents. Existing SQLite doctor may still fail.
- Keep the async command entry and item dispatcher boxed. Real Windows process
  validation found stack exhaustion not reproduced on Rust test threads.
  Also box the large create/add-doi/add-url/add-file/attach futures inside the
  item dispatcher: boxing only its caller leaves ingestion state and debug
  construction temporaries in every item's polling path. A real tag preview
  overflowed despite passing mocked tag tests. Keep the item dispatch future
  under the 16 KiB regression budget and verify production CLI HTTP paths.

## 4. Validation & Error Matrix

| Condition | Result / write behavior |
| --- | --- |
| Missing field input, empty strings, wrong schema/creator shape | `local-write-empty/field/creators`, before auth |
| Blank or missing tag name | `local-write-tag`, before auth |
| Illegal server tag array/object/type | `local-api-protocol`, before auth; do not PATCH `[]` |
| Old Zotero/missing API or Server ID | `local-api-unsupported` |
| Wrong server/stale local version | `local-api-conflict`, before auth or HTTP 412 |
| Cross-library/missing target | HTTP 404 -> `local-api-not-found` |
| Unauthorized key | 401 -> one reauthorization; second error surfaces |
| Denial/disabled API/non-writable library | 403 -> `local-api-forbidden`, no retry |
| Missing preconditions | 428 -> `local-api-precondition` |
| Authorization rate limit | 429 -> `local-api-rate-limit`, Retry-After hint |
| Timeout/5xx after PATCH | `unknown`, readback only; no automatic retry |
| Accepted PATCH, readback mismatch/unavailable | `verification-failed`, accepted true |
| Accepted PATCH, matching readback | `applied`, verified true |
| Edited organize counts/flags/requests/fields | schema/conflict error before auth |
| No-op version or referenced collection drifts | conflict; stop subsequent items |
| Report save fails before submission | current and remaining items not-attempted; non-zero exit |
| Report save fails after submission/finalization | known stdout results retained + report error; disk may remain in-flight; non-zero exit |
| Collection POST buckets conflict or contain malformed/extra indexes | unknown; never claim success or resend |
| Local collection delete hash/server/version mismatch | `local-delete-descendants` or `local-api-conflict`, before auth and DELETE |
| DELETE 204 with the same Server ID, then GET 404 | `applied`, verified true |
| DELETE HTTP 200, timeout, 5xx, or mismatched Server ID | `unknown`; do not GET and then claim success |
| DELETE 404 | `absent`; this call did not delete the object |
| DELETE 412 | `local-api-conflict`, not unknown |

## 5. Good / Base / Bad Examples

- Good: item in A/C, move A to B -> B/C with one conditional PATCH, same key.
- Good: add `待读` beside an automatic tag; PATCH sends the complete tag array,
  preserves type 1 and unknown properties, and accepts a reordered readback.
- Base: fill blank DOI; existing title appears in skipped_fields. No auth during preview.
- Base: adding an exact existing name, or removing a missing name, is unchanged
  and does not authorize or PATCH.
- Bad: pass creators or tags through `--field`, silently overwrite title, use Web version
  for a local item, replace malformed tags with `[]`, or treat `unknown` with
  observed_expected_state=true as success.

## 6. Tests Required

- `local_api_tests.rs`: identities/versions, version gates, group scope, exact
  headers/minimal body, auth redaction, redirects and HTTP errors.
- `local_write_tests.rs`: membership semantics, fill/overwrite/reserved fields,
  structured creators, tag diff/readback, clap guards, no-op/preview no-auth,
  stale previews, cached auth recovery, unknown/verification-failed and readback order.
- `local_credentials.rs` tests: profile/server partitioning, redaction, atomic
  persistence and Unix modes. `json_error_contract.rs`: real process validation.
- `just ci`; bilingual docs build; canonical skill mirror check. Real Zotero
  preview is separate evidence from authorization/mutation/UI verification.
- `organize_tests.rs`: three distinct successful decisions, 51-item rejection
  and explicit raised-limit apply, forged plans, no-op/collection drift, 401,
  permission rejection, timeout with matching readback, real filesystem save
  failure, and a killed test-harness process leaving in-flight evidence. Test
  harness subprocess evidence is not production CLI or real desktop evidence.
- For desktop acceptance, create a separate profile **and** explicit data
  directory. A new profile alone does not prove library isolation. Bind fixture
  keys to the verified Server ID, keep synthetic data under ignored `target/`,
  and use the production CLI endpoint on port 23119. Restore the original
  instance afterward. Never copy the user's library or authorization file.
- The user performs Deny/Allow/Always Allow. Check both API snapshots and UI,
  unchanged child objects, stale previews after UI edits, and cached-key reuse
  after restart. Windows Zotero 10.0.6 passed this baseline on 2026-10-13;
  real group permissions and interrupted submissions remain separate evidence.
- A sandbox denial when saving a remembered key is an environment I/O failure,
  not a Zotero permission rejection. Credential persistence happens before
  PATCH; obtain the necessary filesystem permission and re-preview. Do not
  bypass persistence or claim the item was changed after that failure.

## 7. Wrong / Correct

Wrong: derive local versions from `zotero.sqlite`, instantiate `ZoteroRemote`
with a localhost URL, remove from A then add to B in two requests, or send only
the newly added tag object.

Correct: read current item/schema via `LocalApiClient`, show one minimal patch,
bind confirmation to local instance/version, send once, and inspect readback.
For tags, the patch is the complete preserved array plus manual additions.
The user approves in Zotero's own authorization dialog; never automate Allow.
