# Design

## Shared Retry And Error Boundary

`http.rs` owns one `send_with_retry(RequestBuilder, code)` path. Before the first send it builds a
clone only to classify the request: `GET`, or any method with `Zotero-Write-Token`, is eligible;
everything else has one attempt. Each attempt comes from `RequestBuilder::try_clone`, preserving
the exact write token and body. An unclonable request is sent once rather than silently changing
semantics.

Eligible responses retry only on 429/5xx, and eligible transport failures retry while attempts
remain. `Retry-After` delta seconds or HTTP date wins over exponential backoff; all delays are
capped, then a small random jitter is added within the same global cap. Responses are dropped
before sleeping so pooled connections are not held. Tests use `Retry-After: 0` or deterministic
pure delay parsing and never wait for production-scale delays.

`ensure_status` reads response chunks only until 4096 bytes, replaces ASCII/control whitespace
with a single space, removes other control characters, and appends a truncation marker. It keeps
the existing `ZotError::Remote` status/hint contract without exposing an unbounded body.

## Zotero Request Contract

`ZoteroRemote` stores a validated fixed `HeaderValue` for API version 3. Its relative-endpoint
builder attaches the key and version to every Zotero API request. The external upload builder is
unchanged in authority: it receives neither header.

All GET call sites and the three write-token creation helpers use `send_with_retry`. Conditional
writes, authorization/register POSTs, and external upload keep direct single sends. This matrix
prevents replaying mutations whose idempotency is not guaranteed while allowing Zotero's write
token contract to protect create retries.

## Untrusted PDF Download

The download implementation lives in `zot-remote` because URL/DNS/HTTP policy is a remote
boundary; `zot-cli` only creates a `NamedTempFile`, asks the remote helper to fill its path, then
passes that path to `upload_attachment`.

```text
parse initial HTTPS URL
  -> validate authority and resolve every host address
  -> GET with redirect disabled
  -> for 3xx: resolve Location against current URL and repeat (max 5)
  -> require 2xx + application/pdf + acceptable Content-Length
  -> stream chunks to temp, cap 100 MiB, capture leading bytes
  -> require first non-empty bytes start with %PDF-
  -> flush/sync, then allow upload
```

The IP predicate is a small pure function covering IPv4 and IPv6 non-public ranges, including
IPv4-mapped IPv6. DNS fails closed if resolution yields no address or any forbidden address. The
client has automatic redirects disabled so no unvalidated hop can run. A test-only policy seam
permits a loopback initial fake server while still rejecting a private redirect; production has
no exception.

`NamedTempFile` supplies same-process cleanup on every early return. The CLI retains the temp
handle through upload, so no guessed temp filename or manual best-effort deletion is needed.

## Attachment Transaction And Memory

Before the first request, metadata verifies a regular file and the 100 MiB cap. A streaming MD5
pass supplies authorization fields without storing the file. If the server needs bytes, one
preallocated payload is built as `prefix`, direct `read_to_end` from the file, then `suffix`; the
file is never first read into a second full `Vec`.

After `create_attachment_item` returns a key, the remaining steps run as one inner operation.
Failure invokes `cleanup_attachment_item`, which fetches the created item's current version and
issues a hard DELETE with `If-Unmodified-Since-Version`. A wrapper preserves the original
`ZotError` category/code/status and augments its hint with `orphan cleanup succeeded` or the
sanitized cleanup error. Cleanup is not retried because it has no write token. Success and
`exists=true` return normally; no compensation runs after completed registration.

## Atom Parser

`quick_xml::Reader` tracks whether it is inside the first Atom `entry`, current author, and the
field being captured. Local names are compared after namespace prefixes. Text and CDATA events
are decoded through quick-xml and accumulated so nested elements do not discard content. The
parser stops after the first entry, normalizes whitespace with the existing helper, and maps all
reader/decode/shape failures to `arxiv-parse`.

DOI/arXiv identifier regexes remain; only XML extraction regexes are removed.

## Compatibility And Rollback

- Public CLI syntax and success payloads do not change. New failures use typed domain errors and
  the already-versioned JSON envelope.
- The dependency change is limited to `quick-xml`; retry and streaming use existing reqwest,
  tokio, chrono, rand, md5, and tempfile capabilities.
- Rollback is the implementation commit. No local persistent schema or user data migration is
  introduced; orphan cleanup only targets the attachment item created by the failing invocation.
- A partial rollback must not restore credential propagation, automatic unvalidated redirects,
  or unbounded body/file buffering.
