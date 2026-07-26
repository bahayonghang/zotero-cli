# Connector Import Contract

Executable contract for `zot item import`, the unauthenticated local write
path that talks to Zotero's own built-in connector HTTP server. Introduced by
task `07-18-connector-local-write`.

## Scenario: import BibTeX/RIS via the built-in connector server

### 1. Scope / Trigger

New command signature (`zot item import`), a new cross-layer HTTP contract
(`zot-desktop` -> Zotero connector server), and a new `ZotError::Connector`
variant — code-spec depth is mandatory. Use this before changing
`zot-desktop/src/connector.rs`, `zot-cli/src/commands/item/import.rs`, or
doctor's `connector_write` capability.

The connector talks to Zotero's own built-in server with **no plugin installed
and no auth**. It is import-only, and the target is whatever the Zotero UI
currently has selected. `connector.rs` is the sole module in `zot-desktop` and
also owns the adjacent read-only `/api/` readiness probe.

### 2. Signatures

```text
zot item import (--file <path> | --text <string>) [--format bibtex|ris] [--confirm]
```

```rust
// zot-desktop/src/connector.rs
impl ConnectorClient {
    pub fn ping(&self) -> ZotResult<ConnectorPing>;
    pub fn probe_local_http(&self) -> ZotResult<LocalHttpStatus>;
    pub fn selected_target(&self) -> ZotResult<SelectedTarget>;
    pub fn import(&self, session: &str, text: &str) -> ZotResult<ConnectorImportResult>;
}
```

```text
GET  /connector/ping
GET  /api/
POST /connector/getSelectedCollection
POST /connector/import?session=<uuid>
```

### 3. Contracts

- Base URL `http://127.0.0.1:23119`, override via `ZOT_CONNECTOR_BASE_URL`;
  loopback-only and validated inside `connector.rs`.
- Header `X-Zotero-Connector-API-Version: 3` on connector requests.
- Timeouts: 5s connect / 30s connector request; `/api/` readiness probes use
  the 5s timeout.
- `SelectedTarget { id, library_id, name, editable, library_editable: Option<bool> }`;
  `library_id` maps connector JSON `libraryID` and participates in target identity;
  writable iff `editable` and (when present) `library_editable` are both
  true — see `SelectedTarget::is_writable()`. Never infer writability from
  only one of the two fields.
- `import` body is raw BibTeX/RIS text, `Content-Type: text/plain`. The
  session id is minted by the CLI caller as `zot-<uuid>` and passed in —
  mirrors `merge_apply(operation_id)`'s caller-minted-id precedent; the
  client itself never generates one.
- Dry-run envelope: `{ target, editable, entries, format, confirmed: false }`
  — no `session`/`status` fields, because nothing was sent.
- Confirmed envelope: `{ session, target, editable, entries, format, status }`.
- Dry-run reads selected target once. Confirm reads it once for the initial gate,
  parses the input, then reads it again immediately before import. The second
  target must still be writable and its
  `(library_id, id, name, editable, library_editable)` fingerprint must match.
  The confirmed envelope reports this revalidated target.
- Format resolution, in priority order: (1) explicit `--format` flag,
  (2) file extension (`.bib`/`.ris`), (3) content sniff (`@\w+\s*\{` prefix
  means bibtex, `^TY  - ` means ris), (4) `connector-import-format` error.
- Entry counting: bibtex via `@\w+\s*\{` match count; RIS via `^TY  - ` line
  count.

### 4. Validation & Error Matrix

| Condition                                                                  | Result                                                             |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Connector unreachable (Zotero not running / port closed)                   | `connector-unreachable`, hint to start Zotero                      |
| Connection timeout                                                         | `connector-timeout`                                                |
| Non-2xx HTTP response                                                      | `connector-http` with `status`                                     |
| Format undetectable from flag/extension/content                            | `connector-import-format`; no network call made                    |
| `--confirm` and (`editable == false` or `library_editable == Some(false)`) | `connector-target-readonly`; import request never sent             |
| confirmed target becomes read-only at the second read                       | `connector-target-readonly`; import request never sent             |
| confirmed target remains writable but fingerprint changes                   | `connector-target-changed`; import request never sent              |
| No `--confirm`                                                             | dry-run only — `ping` + `selected_target` run, `import` never sent |
| `--file` and `--text` both given, or neither given                         | clap-level error (`conflicts_with` / `required_unless_present`)    |

### 5. Good / Base / Bad Cases

- Good: Zotero running, the same writable library/collection is observed twice,
  `--confirm` is given -> entries land in that target and the envelope reports
  `session`/`status`.
- Base: no `--confirm` -> dry-run reports target/editable/entries/format,
  zero network writes.
- Bad: a read-only group/feed is selected and `--confirm` is given ->
  `connector-target-readonly` before any import request leaves the process.
- Bad: the user switches library/collection while confirm input is being
  prepared -> `connector-target-changed` and zero import requests.
- Bad: Zotero is closed -> `connector-unreachable`; no Web API fallback
  under any connector failure, ever.

### 6. Tests Required

- `zot-desktop`: tiny_http fake-server tests for ping success/non-2xx/
  timeout, selected-target writable/readonly/non-2xx, import success (JSON
  and non-JSON response body)/non-2xx, and non-loopback base URL rejection.
- `zot-cli`: format-sniff and entry-counting unit tests; a scripted fake
  server that proves dry-run sends exactly `ping` + `selected_target` (a
  stray `import` call hits connection-refused and fails the test, not just
  a wrong output field); the same proof-by-absence shape for the
  readonly-target gate; a confirmed-writable-target happy path asserting
  two selected-target reads and the returned `session`/`status`; changed
  library/collection and second-read readonly fixtures proving zero import.
- `just ci` full gate (fmt / check / clippy `-D warnings` / test /
  skills-check).

### 7. Wrong vs Correct

```rust
// Wrong — accepts an arbitrary remote base URL for a local unauthenticated
// write transport:
let base_url = Url::parse(raw)?;

// Correct — connector owns and enforces the loopback constraint:
fn parse_connector_base_url(raw: &str) -> ZotResult<Url> { /* local copy */ }
```

```rust
// Wrong — sends the import request, then inspects the response for
// writability:
let result = client.import(&session, &text)?;
if !target.is_writable() { return Err(readonly_error()); }

// Correct — the readonly gate runs before any import call, so a read-only
// or changed target never reaches the import endpoint:
if !target.is_writable() {
    return Err(readonly_error());
}
let confirmed = client.selected_target()?;
if !confirmed.is_writable() {
    return Err(readonly_error());
}
if target_fingerprint(&confirmed) != target_fingerprint(&target) {
    return Err(changed_target_error());
}
client.import(&session, &text)?;
```
