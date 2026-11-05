# Desktop Library Reads

## 1. Scope / Trigger

Use when editing open-library reads for `item get`, `item children`,
`item pdf` / `item fulltext`, `library search`, or `library citekey`,
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

LocalApiClient::probe_for_read() -> ZotResult<LocalApiProbe>   # 2s, GET /api/
LocalApiClient::read_exchange(path, server_id) -> ZotResult<LocalReadExchange>
DesktopRead::{item_view, children_for_keys, local_pdf_path, search, citation_key_matches}
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
  unless the existing trash flag says otherwise.
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
  opens and zero authorize calls.
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

## 7. Wrong / Correct

#### Wrong

Follow a Local API or Better BibTeX redirect, call authorize for a GET, copy
the locked database, or treat a missing `Last-Modified-Version` as one commit.
Replace `desktop-read-bindings.json` with `rename` alone on Windows.

#### Correct

Refuse the redirect, send the known server id without a write key, and keep
the existing snapshot backup for the closed-library path. Retry a missing or
changed library version, then fail `desktop-read-inconsistent`. Delete the
binding destination on Windows before renaming the temporary file into place.
