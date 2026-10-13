# Local API Content Operations

## 1. Scope / Trigger

Use when editing `zot-desktop/src/local_api.rs`, CLI `local_write.rs`,
`local_credentials.rs`, local command flags, or `doctor.local_api_write`.
Zotero 10+ official Local API writes are separate from connector imports,
SQLite reads, and the Web API. Evidence: task `10-13-zotero-local-content-operations`.

## 2. Signatures

```text
item update KEY --write-backend local [--title VALUE] [--date VALUE]
  [--field NAME=VALUE ...] [--creators-file FILE] [--overwrite]
collection add-item COLLECTION ITEM --write-backend local
collection remove-item COLLECTION ITEM --write-backend local
collection move-item ITEM --from SOURCE --to TARGET --write-backend local

All local operations: [--confirm --expected-server-id ID --expected-version N]
GET /api/
GET /api/itemTypeFields?itemType=TYPE
GET /api/itemTypeCreatorTypes?itemType=TYPE
GET /api/{users/0|groups/ID}/{items|collections}/KEY
POST /api/local/authorize {"appName":"zot CLI"}
PATCH /api/{users/0|groups/ID}/items/KEY
```

PATCH includes `Zotero-Server-ID`, `Zotero-API-Key`,
`If-Unmodified-Since-Version`, and JSON `version` plus changed fields only.
`LocalObject` validates outer/data key and version consistency. Keys are eight
uppercase ASCII letters/digits. Group IDs are public IDs from `LibraryScope`.

## 3. Contracts

- Production endpoint is fixed loopback HTTP port 23119. No proxies or redirects.
  Alternate origins exist only through the `test-support` constructor.
- Probe requires Zotero major >=10, API 3, and nonempty Server ID. Server identity
  is checked on every read and successful write. Local versions never come from
  Web or SQLite. Local commands never call `ctx.remote()`/`ctx.local_library()`.
- Existing update/add/remove default to Web. Move is explicitly local only.
  Web rejects local confirmation, overwrite, and creators-file flags.
- Preview/no-op never reads credential contents, authorizes, or sends PATCH.
  Confirmation re-reads the item and rejects stale server/version before auth.
  Use the same library, key, mutations, and flags as the reviewed preview.
- Only non-trashed, top-level bibliographic items. Validate collection keys in
  the same library. Move is `(members - source) + target`, in one PATCH;
  preserve all other memberships. Repeated move to an existing target is no-op.
- Fields must be in the item's current schema and not reserved identity/array
  fields. Strings cannot be cleared. Fill empty/missing/whitespace-only values
  by default; nonempty differing values are skipped unless `--overwrite`.
- Creators are a nonempty complete ordered array, with schema-valid roles and
  either `name` or `firstName`/`lastName`. Do not concatenate, infer, or dedupe.
  Replacing a nonempty array requires overwrite. Tags/notes/attachments and
  itemType conversion are excluded. Native `citationKey` is schema-dependent.
- Authorization occurs only for a confirmed change. `SecretString` prevents
  Debug leakage. Remembered keys are partitioned by effective profile + Server
  ID in `AppConfig::config_dir()/local-api-credentials.json`; no temp/CWD
  fallback. `write_secret_atomically` uses restrictive atomic persistence.
  One-time keys remain in process memory. Never log request keys/response bodies.
- A 401 can invalidate the cached key, authorize, and retry once. No retries
  after timeout, 5xx, or unreliable successful responses: report `unknown` and
  attempt readback. Never switch backends. A 204 from the expected server is
  accepted; it becomes `applied` only after matching same-server readback.
- Report fields include `backend`, `state`, `server_id`, `library`, `item_key`,
  `expected_version`, `before`, `after`, `skipped_fields`, collection names,
  `write_accepted`, `verified`, `observed_expected_state`, and optional `error`.
  Collection arrays compare as memberships; creator arrays compare in order.
- The existing JSON envelope stays at API version 1. A completed report uses
  `ok:true` even for `unknown` / `verification-failed`; callers must check state
  and verified. Pre-submission/rejection errors use `ok:false` and exit 1.
- Doctor only reports reachability/support/credential presence; `verified` is
  false. It never authorizes or test-writes. Secret cache decode errors are
  generic and never expose contents. Existing SQLite doctor may still fail.
- Keep the async command entry and item dispatcher boxed. Real Windows process
  validation found stack exhaustion not reproduced on Rust test threads.

## 4. Validation & Error Matrix

| Condition | Result / write behavior |
| --- | --- |
| Missing field input, empty strings, wrong schema/creator shape | `local-write-empty/field/creators`, before auth |
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

## 5. Good / Base / Bad Examples

- Good: item in A/C, move A to B -> B/C with one conditional PATCH, same key.
- Base: fill blank DOI; existing title appears in skipped_fields. No auth during preview.
- Bad: pass creators through `--field`, silently overwrite title, use Web version
  for a local item, or treat `unknown` with observed_expected_state=true as success.

## 6. Tests Required

- `local_api_tests.rs`: identities/versions, version gates, group scope, exact
  headers/minimal body, auth redaction, redirects and HTTP errors.
- `local_write_tests.rs`: membership semantics, fill/overwrite/reserved fields,
  structured creators, clap guards, no-op/preview no-auth, stale previews,
  cached auth recovery, unknown/verification-failed and readback order.
- `local_credentials.rs` tests: profile/server partitioning, redaction, atomic
  persistence and Unix modes. `json_error_contract.rs`: real process validation.
- `just ci`; bilingual docs build; canonical skill mirror check. Real Zotero
  preview is separate evidence from authorization/mutation/UI verification.

## 7. Wrong / Correct

Wrong: derive local versions from `zotero.sqlite`, instantiate `ZoteroRemote`
with a localhost URL, or remove from A then add to B in two requests.

Correct: read current item/schema via `LocalApiClient`, show one minimal patch,
bind confirmation to local instance/version, send once, and inspect readback.
The user approves in Zotero's own authorization dialog; never automate Allow.
