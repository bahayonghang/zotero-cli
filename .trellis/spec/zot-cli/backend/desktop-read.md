# Desktop Library Reads

## 1. Scope / Trigger

Use when editing open-library reads for `item get`, `item children`,
`item pdf` / `item fulltext`, `library search`, `library citekey`,
`collection list`, `collection get`, `collection subcollections`,
`collection search`, `collection items`, `collection item-count`,
`collection tags`, `library list`, `library stats`, or `library tags`,
including `zot-cli/src/commands/desktop_read.rs`,
`zot-desktop/src/local_read.rs`, the read side of
`zot-desktop/src/local_api.rs`, citation-key lookup in `zot-local`,
or the loopback Better BibTeX `item.search` client.

These reads are separate from authenticated Local API writes in
[local-api.md](./local-api.md) and from connector import. Evidence:
task `11-04-zotero-open-library-reads`.

## 2. Signatures

```text
item get KEY [--running-zotero]
item children KEY... [--running-zotero]
item pdf KEY [--pages RANGE] [--annotations] [--running-zotero]
item fulltext KEY [--pages RANGE] [--annotations] [--running-zotero]
library search [QUERY] [--title TEXT] [--journal TEXT] [--topic TEXT] [--running-zotero]
library citekey CITEKEY [--running-zotero]
collection list|get|subcollections|search|items|item-count|tags [--running-zotero]
library list|stats|tags [--running-zotero]

LocalApiClient::probe_for_read() -> ZotResult<LocalApiProbe>   # 2s, GET /api/
LocalApiClient::read_exchange(path, Option<&str>) -> ZotResult<LocalReadExchange>
DesktopRead::{item_view, children_for_keys, local_pdf_path, search, citation_key_matches,
              collection_list, collection_get, collection_subcollections, collection_search,
              collection_items, collection_item_count, collection_tags,
              library_list, library_stats, library_tags}
```

`QUERY` may be empty when a field flag is present. `--running-zotero` is the
only opt-in for an unproved running instance. Binding file:
`AppConfig::state_dir().join("desktop-read-bindings.json")`, with entries
`{ "data_dir", "server_id" }`.

## 3. Contracts

- One command uses one source. `meta.read_source` is `desktop`,
  `desktop-unbound`, or `snapshot`. The field is optional and omitted when
  absent. `api_version` stays 1. Human output has no `Source:` line.
- Default route probes Local API v3 first. `probe_for_read` times out in 2
  seconds and does not change the 30-second write probe. A closed process,
  unavailable service, or forbidden probe may open the configured snapshot
  once. Identity mismatch does not. Object 404, HTTP 412, redirect, and file
  errors do not.
- A nonempty generic `library search` query is snapshot-first, so body
  fulltext stays on the SQLite index when that same library opens. If that
  snapshot fails and the running instance is a mismatch or unbound, return
  `library-read-unavailable` with both reasons. Do not read the other source.
  A locked snapshot that then uses a proved desktop reports
  `fulltext_index=unavailable`.
- `users/0` is the running instance. With a profile, `ZOT_DATA_DIR`, or a
  configured data directory, automatic desktop reads require a stored
  `localAPI/serverID` equal to the response `Zotero-Server-ID`. The id is
  saved only after a successful snapshot. Do not snapshot a locked library on
  every call to refresh it. On Windows, delete the destination before
  `rename`, because `rename` does not replace an existing file. No instance
  id means no automatic binding. `--running-zotero` then uses
  `desktop-unbound` and skips the snapshot.
- Ordinary GET sends `Zotero-API-Version: 3` and, when known, the server id.
  It never calls authorize and never sends the write key. Redirects are
  refused. Each multi-GET command records `Last-Modified-Version`. A missing
  version, a version change, or a changed `Total-Results` retries up to 3
  times and then returns `desktop-read-inconsistent`.
- Paged scans use 100 rows and stop after 20 pages. A short page, a missing
  next link, or the page cap is `desktop-search-incomplete` when the declared
  total exceeds the rows collected, or when the cap is hit. That error does
  not report a total or an empty result. Field filters run before the final
  page. Journal matches `publicationTitle` or `journalAbbreviation`. Topic
  matches title, abstract, or tag text. Trash and attachments stay excluded
  unless the existing trash flag says otherwise. `DesktopScanLimits.max_pages`
  stays 20. Inventory reads do not use that cap.
- Collection and library inventory reads always send `limit=100` and `start`
  through `read_exchange`. `desktop-unbound` passes `server: None` and sends
  no `Zotero-Server-ID`. The cap is 100 pages (10,000 rows). Exactly 10,000
  succeeds. A 10,001st row, a next link after 10,000, or a short page whose
  `Total-Results` exceeds the rows collected returns `desktop-read-incomplete`
  with no `total` and no partial list. The message does not invent a library
  total. Version or `Total-Results` drift retries the whole read up to 3
  times, then `desktop-read-inconsistent`. That is not `local-api-conflict`.
  HTTP 412 and redirects stay immediate and do not switch source.
- Do not call `collection_snapshot` for these reads. It remains the
  write-preview helper: it requires `server: &str` and returns
  `local-api-conflict` on version drift.
- Collection items, item-count, and collection tags first resolve the
  argument from the collection list: exact key, otherwise one name, otherwise
  `collection-ambiguous`. Only a confirmed key is then read from
  `GET {prefix}/collections/{key}/items/top?includeTrashed=1&limit=100`.
  On Zotero 10.0.8 an unknown key becomes `collectionID` false. Looking that
  id up returns false, and destructuring it throws; the adjacent branch
  returns without adding the condition when the lookup has no key. The member
  URL is therefore not a not-found signal, and a skipped condition is HTTP
  200 for the whole top-level library. Do not trust that response. Do not use non-top
  `/items` totals or `/collections/{key}/tags`.
  Annotations in that payload are dropped. Notes and attachments stay.
  A child note is absent, so its unique tag is absent. Count is the length
  of that set. Desktop member commands set `meta.direct_annotations` to
  `omitted`. Snapshot collection SQL can still return a direct annotation.
  A child row that also has its own `collectionItems` row stays in the
  snapshot and is absent from `/items/top`. Do not open SQLite to fill it.
  Desktop member order follows the API page. Snapshot item order follows
  `orderIndex`. Collection tree order is not part of this contract.
- `collection get` takes its node from that same paged collection list. A
  single-object response carries the collection's own version, while the list
  carries the library version. Do not compare those two headers. An object
  version of 7 and a library version of 45, with no edit during the read, is
  success.
- Collection list, get, subcollections, and search follow the API collection
  list, which omits trashed collections. Desktop tree commands set
  `meta.trashed_collections` to `omitted`. Closed SQLite collection list
  behavior stays unchanged. Do not open SQLite to fill either gap.
- `library list` requests `/items/top` or
  `/collections/{key}/items/top` with
  `itemType=-attachment&itemType=-note&itemType=-annotation` before applying
  the 10,000-row cap. The cap is the candidate total after those constraints,
  not the unfiltered library total. 10,000 bibliographic rows succeed across
  100 pages of 100. One extra attachment, or a collection that holds one
  article inside a larger library, does not trip the cap. Sort the candidate
  set by key descending locally, then apply offset/limit. Never send
  `sort=key`. `--collection` accepts a key or
  a unique name. Duplicate names return `collection-ambiguous` with keys in
  ascending order. Resolve the name from the same source's collection list,
  including rows the tree drops. Do not open a second snapshot.
- `library tags` reproduces `get_tags()` (every item tag, including notes,
  attachments, annotations, and trash), count descending and name ascending.
  `library stats` reproduces `get_stats_with_trashed`. `top_tags` stays
  bibliographic. `pdf_attachments` counts distinct bibliographic parents with
  `application/pdf`. `notes` counts note rows under bibliographic parents.
  Trash for those two fields follows the parent. Both fetches send
  `includeTrashed=1` and filter locally. Do not change the SQL.
  `dateAdded` and `dateModified` use the snapshot spelling:
  `YYYY-MM-DD HH:MM:SS`. Zotero's `sqlToISO8601` only inserts `T` and `Z`,
  so reverse that spelling and do not shift the clock. Compare the full item
  JSON on one fixture.
- `get_collection_items` stays bibliographic. Collection item commands use
  `get_collection_direct_items`, which keeps parentless notes and attachments.
  Workspace import, semantic indexing, and Scite must not switch to the
  direct set.
- `group:<id>` uses `prefix(scope)`. Identity mismatch without
  `--running-zotero` is `desktop-library-mismatch` and does not read the
  other source. Collection 404 is `collection-not-found` and does not fall
  back. A collection list HTTP 404 is `local-api-http`. Probe success does
  not increase `snapshot_open_attempts`. Probe failure uses the
  `LocalLibrary` already inside `Route::Snapshot` and increases that counter
  by exactly one.
- Citation key match is exact and case-preserving. Closed snapshots query the
  native `citationKey` field and Extra `Citation Key:` / `CitationKey:` lines.
  Open reads use those Local API fields, then a loopback Better BibTeX
  `item.search` candidate only when the already-scoped item carries that key.
  Dedupe by item key. Several items produce `citation-key-ambiguous` and list
  `title | author | year | key`. BBT `libraryID` is not a group id. Only HTTP
  loopback is called. An external `ZOT_BBT_URL` is not requested.
- PDF selection stays the first `application/pdf`. The path comes from
  `items/{attachment}/file/view/url`, is converted as a local file URL, and
  must be a non-symlink file of a PDF type. Network URLs are not followed.
  Extraction and cache stay in `zot-local`.
- `doctor` adds `sqlite_snapshot` and `desktop_read` beside the existing
  errors. Both set `item_read_verified` and `pdf_read_verified` to false.
  A reachable probe is not a successful item or PDF read. Read commands do
  not require a prior doctor.

## 4. Validation & Error Matrix

| Condition | Code | Other source |
| --- | --- | --- |
| Probe down or 403, snapshot opens | success, `read_source=snapshot` | one snapshot |
| Probe down or 403, snapshot fails | `library-read-unavailable` | both reasons, stop |
| Stored server id differs, no flag | `desktop-library-mismatch` | no |
| Same mismatch after a snapshot-first failure | `library-read-unavailable` | no item read |
| No proved binding or no instance id, no flag | snapshot, or `library-read-unavailable` | one snapshot |
| `--running-zotero` without proof | success, `read_source=desktop-unbound` | no snapshot |
| Item 404 | `item-not-found` | no |
| HTTP 412 | `local-api-conflict` | no |
| HTTP 3xx | `local-api-redirect` | no |
| Missing or changed library version after 3 tries | `desktop-read-inconsistent` | no |
| Declared total exceeds collected rows, or 20-page cap | `desktop-search-incomplete` | no total |
| Inventory read above 10,000 rows | `desktop-read-incomplete` | no total, no partial list |
| Inventory version or `Total-Results` drift after 3 tries | `desktop-read-inconsistent` | no |
| Collection object or member 404 | `collection-not-found` | no |
| Collection list HTTP 404 | `local-api-http` | no |
| File URL 405 or 501 | `desktop-file-url-unsupported` | no |
| File URL 404 | `desktop-file-missing` | no |
| Symlink or non-file PDF path | `desktop-file-type` | no |
| Several stored items for one citekey | `citation-key-ambiguous` | candidates in the message |
| No stored key and plugin unreachable | `citation-key-source-limited` | no |
| Plugin candidate not stored on the scoped item | `citation-key-unverified` | no |
| Reachable plugin returns no exact key | `citation-key-not-found` | no |
| Plugin redirect | `bbt-redirect` | no |

## 5. Good / Base / Bad Cases

- Good: Zotero 10+ holds the exclusive lock, the server id matches the stored
  binding, and `item get` returns the committed fields with zero snapshot
  opens and zero authorize calls. The same lock still serves `collection list`
  and `library list` from the desktop, with `snapshot_open_attempts` unchanged.
- Base: Zotero is closed. The same command uses one consistent snapshot and
  reports `read_source=snapshot`.
- Bad: the running server id does not match, and the command reads either
  library without `--running-zotero` or reports a total after a short page.

## 6. Tests Required

- Desktop hit: zero snapshot opens and zero authorize or write requests.
- Held exclusive connection still completes get, children, and page-1 PDF
  text through the desktop route.
- Group same-key, profile or data-dir mismatch, `--running-zotero`, version
  change, 403, closed process, redirect, and 404 stay on the selected source.
- `citation-key-ambiguous` includes both item keys and titles.
- Binding file round-trips a replacement server id on Windows semantics.
- Search covers case, whitespace, title fragment, journal full name and
  abbreviation, abstract or tag hits, body-only hits, trash exclusion, and a
  cross-page total that must not be reported when the walk stops early.
- Open and closed runs of the same fixture agree on candidates. A generic
  query does not mix a snapshot fulltext hit with desktop metadata.
- Inventory: one article plus one parentless attachment is count 2, and a
  tag that exists only on the child note is absent. `library tags` and
  `library stats` keep their existing SQL counts on the same fixture.
- A complete read sends `limit=100`, accepts exactly 10,000 rows, and rejects
  10,001 with `desktop-read-incomplete` and no total. `sort=key` is absent.
  Version drift retries 3 times into `desktop-read-inconsistent`, not
  `local-api-conflict`. Search `max_pages` stays 20.
- Probe success leaves `snapshot_open_attempts` unchanged. Probe failure
  uses the library inside `Route::Snapshot` and increases it by one.
  `--running-zotero` is absent from write commands.

## 7. Wrong / Correct

#### Wrong

Follow a Local API or Better BibTeX redirect, call authorize for a GET, copy
the locked database, or treat a missing `Last-Modified-Version` as one commit.
Replace `desktop-read-bindings.json` with `rename` alone on Windows.
Read inventory through `collection_snapshot`, send `sort=key`, or use a
non-top `/items` total as the collection member count.

#### Correct

Refuse the redirect, send the known server id without a write key, and keep
the existing snapshot backup for the closed-library path. Retry a missing or
changed library version, then fail `desktop-read-inconsistent`. Delete the
binding destination on Windows before renaming the temporary file into place.
