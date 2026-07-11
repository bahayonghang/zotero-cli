# Desktop Bridge Contract

## Scenario: Authenticated Zotero Desktop Bridge

### 1. Scope / Trigger

Use this contract when changing `plugins/zot-bridge/`, `src/zot-desktop`, the
`zot bridge` command family, bridge configuration, or doctor capability output.
The bridge is a loopback-only, allowlisted write transport. It must never become
a generic script, eval, or arbitrary field-update surface.

### 2. Signatures

CLI surface:

```text
zot bridge setup [--output <dir>]
zot bridge pair <code>
zot bridge status
zot bridge revoke [--local-only]
```

Protocol v1 surface, all JSON POST requests:

```text
http://127.0.0.1:23119/zot-bridge/v1/health
http://127.0.0.1:23119/zot-bridge/v1/pair
http://127.0.0.1:23119/zot-bridge/v1/status
http://127.0.0.1:23119/zot-bridge/v1/auth/revoke
```

`src/zot-desktop::DesktopClient` owns HTTP and DTO decoding. `zot-cli` owns
configuration mutation and output. The plugin owns Zotero lifecycle, auth, and
endpoint registration. `zot-local` and `zot-remote` must not depend on this
transport.

### 3. Contracts

Every request contains `protocol_version`, `request_id`, `sent_at`, and
`client { name, version }`. Protected requests use `Authorization: Bearer`.
Responses use the standard `ok/data/meta` or `ok/error/meta` shape; response
`request_id` and `protocol_version` must match the request and client.

Pairing codes are eight-character CSPRNG values, live in plugin memory for five
minutes, and are single-use. Only Zotero UI actions may create or reveal a code.
`/pair` validates `ZOT_BRIDGE.pairing`; it must never call
`ensurePairingCode()` to create an undisplayed replacement. The plugin stores
only the token SHA-256 hash; CLI config stores the token and never emits it.

Config defaults to `write_backend = "web"` for old files. Successful pairing
sets the selected config target to `desktop`. Target precedence is explicit
profile, default profile, then root. A command-line backend override is not
persisted.

Doctor reports `local_sqlite_read`, `local_http_read`, `desktop_write`, and
`web_write` independently. Disabling the plugin may make `desktop_write`
unavailable while Local HTTP and SQLite remain available.

### 4. Validation & Error Matrix

| Condition | Required result |
| --- | --- |
| Missing, used, or expired displayed code | `401 bridge-pair-expired` |
| Active displayed code does not match | `401 bridge-pair-code` |
| Missing or wrong bearer token | `401 bridge-auth` |
| Browser `Origin` present | reject before operation (`bridge-origin` in handler tests) |
| Host outside loopback allowlist | reject before operation (`bridge-host` in handler tests) |
| Unknown JSON field | `400 bridge-unknown-field` |
| Reused request ID with different payload/auth | `409 bridge-replay` |
| Protocol mismatch | `409 bridge-protocol` or client `bridge-protocol` |
| Endpoint absent while Zotero Local HTTP responds | `bridge-not-installed` |
| Invalid success JSON | client `bridge-invalid-response` |
| Response over 64 KiB | client `bridge-response-too-large` |

### 5. Good / Base / Bad Cases

- Good: UI shows a code, `bridge pair` exchanges it once, `bridge status`
  authenticates, and doctor reports an editable desktop library.
- Base: no token is configured; health still identifies the plugin and status
  reports installed but not paired with a recovery hint.
- Bad: a browser request, remote host, stale code, wrong token, replay conflict,
  incompatible protocol, or arbitrary field is rejected without mutation.

### 6. Tests Required

- `cargo test -p zot-desktop`: health/pair/status/revoke, bearer capture, 401,
  timeout, invalid JSON, oversize, replayed response, request ID mismatch, and
  protocol mismatch.
- `node --test plugins/zot-bridge/tests/bootstrap.test.cjs`: Origin/unknown
  fields, five-minute expiry state, single use, replay conflict/cache, revoke,
  and shutdown endpoint removal.
- `just xpi-check`: allowlisted assets plus manifest/plugin/workspace versions.
- CLI/config tests: parse surface, target precedence, migration default, secret
  redaction, doctor independence, and stable hints.
- Real Zotero smoke: install/restart, pair/status, 306-second expiry, revoke and
  re-pair, wrong token, plugin disable -> endpoint 404 while Local HTTP remains
  available, re-enable, and final doctor status.

### 7. Wrong vs Correct

Wrong: generate a replacement inside `/pair`; the client can never know that
hidden code and the expired branch becomes unreachable.

```javascript
const pairing = await this.ensurePairingCode();
```

Correct: pairing creation belongs to Zotero UI startup/menu paths; `/pair`
only consumes the currently displayed in-memory record.

```javascript
const pairing = this.pairing;
if (!pairing || pairing.used || Date.now() > pairing.expiresAt) {
  throw this.error(401, "bridge-pair-expired", "Pairing code expired", hint);
}
```
